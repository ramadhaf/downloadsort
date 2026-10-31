use crate::config::Config;
use crate::move_file;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInstallPlan {
    pub app_name: String,
    pub install_dir: PathBuf,
    pub binary: PathBuf,
    pub icon: String,
    pub desktop_file: PathBuf,
}

pub enum AppAction {
    IgnoredNonApp,
    Installed(AppInstallPlan),
    PlannedInstall(AppInstallPlan),
}

struct ArchiveInspection {
    is_app: bool,
    planned_exec_rel: Option<PathBuf>,
    planned_icon_rel: Option<PathBuf>,
}

pub fn is_tar_gz_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".tar.gz") || lower.ends_with(".tgz")
}

pub fn derive_app_name(filename: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    let stem = if let Some(s) = lower.strip_suffix(".tar.gz") {
        s
    } else if let Some(s) = lower.strip_suffix(".tgz") {
        s
    } else {
        lower.as_str()
    };

    let mut name_parts = Vec::new();
    for token in stem.split(['-', '_']) {
        if token.is_empty() {
            continue;
        }
        if is_version_or_arch_token(token) && !name_parts.is_empty() {
            break;
        }
        name_parts.push(token);
    }

    if name_parts.is_empty() {
        stem.to_string()
    } else {
        name_parts.join("-")
    }
}

fn is_version_or_arch_token(token: &str) -> bool {
    if token.starts_with("linux")
        || matches!(
            token,
            "x64"
                | "x86"
                | "x86_64"
                | "amd64"
                | "arm64"
                | "aarch64"
                | "64bit"
                | "32bit"
                | "gnu"
                | "musl"
                | "portable"
                | "bin"
        )
    {
        return true;
    }
    let trimmed = token.strip_prefix('v').unwrap_or(token);
    !trimmed.is_empty()
        && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == 'a' || c == 'b' || c == 'r' || c == 'c')
}

pub fn format_display_name(app_name: &str) -> String {
    app_name
        .split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut s = first.to_uppercase().to_string();
                    s.push_str(chars.as_str());
                    s
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn inspect_archive(archive_path: &Path, app_name: &str) -> Result<ArchiveInspection, String> {
    let output = Command::new("tar")
        .arg("-P")
        .arg("-ztvf")
        .arg(archive_path)
        .output()
        .map_err(|e| format!("Failed to invoke tar to inspect {}: {e}", archive_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to inspect archive {}: {}",
            archive_path.display(),
            stderr.trim()
        ));
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    let mut all_entries: Vec<(String, String)> = Vec::new();
    let mut regular_members: Vec<(bool, u64, String)> = Vec::new();

    for line in listing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(entry) = parse_tar_tvf_line(trimmed) else {
            continue;
        };

        if is_unsafe_member_path(entry.member_path) {
            return Err(format!(
                "Unsafe path traversal entry '{}' in {}",
                entry.member_path,
                archive_path.display()
            ));
        }
        if let Some(target) = entry.link_target {
            if is_unsafe_link_target(entry.member_path, target) {
                return Err(format!(
                    "Unsafe link target '{target}' for '{}' in {}",
                    entry.member_path,
                    archive_path.display()
                ));
            }
        }

        all_entries.push((entry.mode.to_string(), entry.member_path.to_string()));

        if entry.mode.starts_with('-') {
            let mode_bytes = entry.mode.as_bytes();
            let has_exec_bit = mode_bytes.len() >= 10
                && (mode_bytes[3] == b'x'
                    || mode_bytes[3] == b's'
                    || mode_bytes[6] == b'x'
                    || mode_bytes[6] == b's'
                    || mode_bytes[9] == b'x'
                    || mode_bytes[9] == b't');
            regular_members.push((has_exec_bit, entry.size, entry.member_path.to_string()));
        }
    }

    let common_root = single_common_root_dir(&all_entries);

    let exec_inputs: Vec<(PathBuf, bool, String)> = regular_members
        .iter()
        .map(|(has_exec_bit, _size, member)| {
            (
                strip_common_root(member, common_root.as_deref()),
                *has_exec_bit,
                member.clone(),
            )
        })
        .collect();
    let planned_exec_rel = select_best_executable(exec_inputs, app_name, |member| {
        member_has_elf_magic_in_archive(archive_path, member)
    });

    let icon_inputs: Vec<(PathBuf, u64)> = regular_members
        .iter()
        .map(|(_has_exec_bit, size, member)| {
            (strip_common_root(member, common_root.as_deref()), *size)
        })
        .collect();
    let planned_icon_rel = select_best_icon(icon_inputs, app_name);

    Ok(ArchiveInspection {
        is_app: planned_exec_rel.is_some(),
        planned_exec_rel,
        planned_icon_rel,
    })
}

fn single_common_root_dir(entries: &[(String, String)]) -> Option<String> {
    let mut root: Option<String> = None;
    for (mode, path_str) in entries {
        let cleaned = path_str.trim_start_matches("./").trim_end_matches('/');
        if cleaned.is_empty() || cleaned == "." {
            continue;
        }
        let first = match cleaned.split_once('/') {
            Some((first, _rest)) => {
                if first.is_empty() {
                    return None;
                }
                first
            }
            None => {
                if !mode.starts_with('d') {
                    return None;
                }
                cleaned
            }
        };
        match &root {
            None => root = Some(first.to_string()),
            Some(existing) if existing != first => return None,
            Some(_) => {}
        }
    }
    root
}

fn select_best_executable<T, F>(
    candidates: Vec<(PathBuf, bool, T)>,
    app_name: &str,
    mut check_elf: F,
) -> Option<PathBuf>
where
    F: FnMut(&T) -> bool,
{
    let lower_app = app_name.to_ascii_lowercase();
    let mut ranked: Vec<(usize, usize, PathBuf)> = Vec::new();

    for (rel_path, has_exec_bit, ctx) in candidates {
        let depth = rel_path.components().count();
        let is_top_level_sh = depth == 1 && is_shell_script_path(&rel_path);
        let is_exec = has_exec_bit || is_top_level_sh || check_elf(&ctx);
        if is_exec {
            let rank = name_match_rank(&rel_path, &lower_app);
            ranked.push((rank, depth, rel_path));
        }
    }

    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    ranked.into_iter().next().map(|(_, _, p)| p)
}

fn select_best_icon(candidates: Vec<(PathBuf, u64)>, app_name: &str) -> Option<PathBuf> {
    let lower_app = app_name.to_ascii_lowercase();
    let mut ranked: Vec<(usize, u64, PathBuf)> = Vec::new();

    for (rel_path, size) in candidates {
        if !is_icon_path(&rel_path) {
            continue;
        }
        let rank = name_match_rank(&rel_path, &lower_app).min(1);
        ranked.push((rank, size, rel_path));
    }

    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    ranked.into_iter().next().map(|(_, _, p)| p)
}

fn strip_common_root(member: &str, common_root: Option<&str>) -> PathBuf {
    let cleaned = member.trim_start_matches("./");
    if let Some(root) = common_root {
        if let Some(rest) = cleaned.strip_prefix(root).and_then(|s| s.strip_prefix('/')) {
            return PathBuf::from(rest);
        }
    }
    PathBuf::from(cleaned)
}

fn is_shell_script_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("sh"))
}

fn is_icon_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("svg"))
}

fn is_elf_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == b"\x7fELF"
}

fn name_match_rank(path: &Path, lower_app: &str) -> usize {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if stem == lower_app {
        0
    } else if stem.contains(lower_app) {
        1
    } else {
        2
    }
}

fn member_has_elf_magic_in_archive(archive_path: &Path, member: &str) -> bool {
    let Ok(mut child) = Command::new("tar")
        .arg("-xOzf")
        .arg(archive_path)
        .arg(member)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };

    let mut magic = [0u8; 4];
    let n = child
        .stdout
        .take()
        .and_then(|mut out| out.read(&mut magic).ok())
        .unwrap_or(0);
    let _ = child.kill();
    let _ = child.wait();

    is_elf_magic(&magic[..n])
}

struct TarEntry<'a> {
    mode: &'a str,
    size: u64,
    member_path: &'a str,
    link_target: Option<&'a str>,
}

fn parse_tar_tvf_line(line: &str) -> Option<TarEntry<'_>> {
    let mut parts = line.split_whitespace();
    let mode = parts.next()?;
    let _owner = parts.next()?;
    let size_token = parts.next()?;
    let _date = parts.next()?;
    let time_token = parts.next()?;

    let size = size_token.parse::<u64>().unwrap_or(0);
    let time_end = (time_token.as_ptr() as usize - line.as_ptr() as usize) + time_token.len();
    let after_time = line[time_end..].trim_start();
    let (member_path, link_target) =
        if let Some((path_part, target)) = after_time.split_once(" -> ") {
            (path_part, Some(target))
        } else if let Some((path_part, target)) = after_time.split_once(" link to ") {
            (path_part, Some(target))
        } else {
            (after_time, None)
        };
    Some(TarEntry {
        mode,
        size,
        member_path,
        link_target,
    })
}

fn is_unsafe_member_path(member_path: &str) -> bool {
    if member_path.starts_with('/') || member_path.starts_with('\\') {
        return true;
    }
    let path = Path::new(member_path);
    path.components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
}

fn is_unsafe_link_target(member_path: &str, target: &str) -> bool {
    if is_unsafe_member_path(target) {
        return true;
    }
    let mut depth = Path::new(member_path)
        .parent()
        .map(|p| {
            p.components()
                .filter(|c| matches!(c, Component::Normal(_)))
                .count()
        })
        .unwrap_or(0);
    for comp in Path::new(target).components() {
        match comp {
            Component::RootDir | Component::Prefix(_) => return true,
            Component::ParentDir => {
                if depth == 0 {
                    return true;
                }
                depth -= 1;
            }
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
        }
    }
    false
}

pub fn handle_archive(
    archive_path: &Path,
    config: &Config,
    dry_run: bool,
) -> Result<AppAction, String> {
    let Some(filename) = archive_path.file_name().and_then(|n| n.to_str()) else {
        return Ok(AppAction::IgnoredNonApp);
    };

    let app_name = derive_app_name(filename);
    let inspection = inspect_archive(archive_path, &app_name)?;
    if !inspection.is_app {
        return Ok(AppAction::IgnoredNonApp);
    }

    let apps_dir = config.resolve_dir(&config.applications);
    let install_dir = apps_dir.join(&app_name);
    let launchers_dir = config.resolve_dir(&config.desktop_launchers);
    let desktop_file = launchers_dir.join(format!("{app_name}.desktop"));

    if dry_run {
        let binary = install_dir.join(
            inspection
                .planned_exec_rel
                .unwrap_or_else(|| PathBuf::from(&app_name)),
        );
        let icon = inspection
            .planned_icon_rel
            .map(|rel| install_dir.join(rel).to_string_lossy().to_string())
            .unwrap_or_else(|| "application-x-executable".to_string());
        return Ok(AppAction::PlannedInstall(AppInstallPlan {
            app_name,
            install_dir,
            binary,
            icon,
            desktop_file,
        }));
    }

    fs::create_dir_all(&apps_dir)
        .map_err(|e| format!("Failed to create applications dir {}: {e}", apps_dir.display()))?;

    let staging_dir = apps_dir.join(format!(".staging-{app_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create staging dir {}: {e}", staging_dir.display()))?;

    let extract_status = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(&staging_dir)
        .status()
        .map_err(|e| format!("Failed to extract {}: {e}", archive_path.display()))?;

    if !extract_status.success() {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!("tar extraction failed for {}", archive_path.display()));
    }

    let top_entries: Vec<PathBuf> = fs::read_dir(&staging_dir)
        .map_err(|e| format!("Failed to read staging dir: {e}"))?
        .filter_map(|r| r.ok().map(|e| e.path()))
        .collect();

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|e| {
            format!(
                "Failed to remove previous installation at {}: {e}",
                install_dir.display()
            )
        })?;
    }

    if top_entries.len() == 1
        && fs::symlink_metadata(&top_entries[0])
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false)
    {
        fs::rename(&top_entries[0], &install_dir).map_err(|e| {
            format!(
                "Failed to move extracted root {} to {}: {e}",
                top_entries[0].display(),
                install_dir.display()
            )
        })?;
        let _ = fs::remove_dir_all(&staging_dir);
    } else {
        fs::rename(&staging_dir, &install_dir).map_err(|e| {
            format!(
                "Failed to move staging dir {} to {}: {e}",
                staging_dir.display(),
                install_dir.display()
            )
        })?;
    }

    let main_exec = find_main_executable(&install_dir, &app_name)
        .ok_or_else(|| format!("No executable found after extracting {app_name}"))?;
    ensure_executable_bit(&main_exec)?;

    let icon = find_best_icon(&install_dir, &app_name)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "application-x-executable".to_string());

    let bin_dir = config.resolve_dir(&config.bin_dir);
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin dir {}: {e}", bin_dir.display()))?;
    let symlink_path = bin_dir.join(&app_name);
    if symlink_path.exists() || fs::symlink_metadata(&symlink_path).is_ok() {
        let _ = fs::remove_file(&symlink_path);
    }
    symlink(&main_exec, &symlink_path).map_err(|e| {
        format!(
            "Failed to create symlink {} -> {}: {e}",
            symlink_path.display(),
            main_exec.display()
        )
    })?;

    fs::create_dir_all(&launchers_dir).map_err(|e| {
        format!(
            "Failed to create desktop launchers dir {}: {e}",
            launchers_dir.display()
        )
    })?;
    let display_name = format_display_name(&app_name);
    let quoted_exec = quote_desktop_exec(&main_exec);
    let desktop_contents = format!(
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName={display_name}\nExec={quoted_exec}\nIcon={icon}\nTerminal=false\nCategories=Utility;Application;\nStartupNotify=true\n"
    );
    fs::write(&desktop_file, desktop_contents).map_err(|e| {
        format!(
            "Failed to write desktop file {}: {e}",
            desktop_file.display()
        )
    })?;

    move_to_trash(archive_path)?;

    Ok(AppAction::Installed(AppInstallPlan {
        app_name,
        install_dir,
        binary: main_exec,
        icon,
        desktop_file,
    }))
}

fn find_main_executable(root: &Path, app_name: &str) -> Option<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(root, &mut files);

    let mut exec_inputs = Vec::new();
    for path in files {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let has_exec_bit = (meta.permissions().mode() & 0o111) != 0;
        exec_inputs.push((rel.to_path_buf(), has_exec_bit, path));
    }

    select_best_executable(exec_inputs, app_name, |path| file_has_elf_magic(path))
        .map(|rel| root.join(rel))
}

fn file_has_elf_magic(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 4];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    is_elf_magic(&buf[..n])
}

fn ensure_executable_bit(path: &Path) -> Result<(), String> {
    let mut perms = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?
        .permissions();
    let mode = perms.mode();
    if (mode & 0o111) == 0 {
        perms.set_mode(mode | 0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to set executable permissions on {}: {e}", path.display()))?;
    }
    Ok(())
}

fn find_best_icon(root: &Path, app_name: &str) -> Option<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(root, &mut files);

    let mut icon_inputs = Vec::new();
    for path in files {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        icon_inputs.push((rel.to_path_buf(), meta.len()));
    }

    select_best_icon(icon_inputs, app_name).map(|rel| root.join(rel))
}

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read_dir.filter_map(|r| r.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            collect_files_recursive(&path, out);
        } else if ft.is_file() {
            out.push(path);
        }
    }
}

fn quote_desktop_exec(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn percent_encode_trash_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::with_capacity(raw.len());
    for &b in raw.as_bytes() {
        let is_unreserved_or_slash = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' | b'/'
            );
        if is_unreserved_or_slash {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn resolve_trash_root() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            return crate::config::expand_tilde(trimmed).join("Trash");
        }
    }
    crate::config::expand_tilde("~/.local/share/Trash")
}

fn move_to_trash(archive_path: &Path) -> Result<(), String> {
    let trash_root = resolve_trash_root();

    let files_dir = trash_root.join("files");
    let info_dir = trash_root.join("info");
    fs::create_dir_all(&files_dir)
        .map_err(|e| format!("Failed to create Trash/files {}: {e}", files_dir.display()))?;
    fs::create_dir_all(&info_dir)
        .map_err(|e| format!("Failed to create Trash/info {}: {e}", info_dir.display()))?;

    let filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid archive filename".to_string())?;

    let mut trash_name = filename.to_string();
    let mut counter = 1usize;
    while files_dir.join(&trash_name).exists() {
        trash_name = format!("{filename}.{counter}");
        counter += 1;
    }

    let trash_file_path = files_dir.join(&trash_name);
    let trash_info_path = info_dir.join(format!("{trash_name}.trashinfo"));
    let deletion_date = current_iso8601_timestamp();
    let abs_archive_path = crate::config::expand_tilde(&archive_path.to_string_lossy());
    let encoded_path = percent_encode_trash_path(&abs_archive_path);
    let info_contents = format!(
        "[Trash Info]\nPath={encoded_path}\nDeletionDate={deletion_date}\n"
    );
    fs::write(&trash_info_path, info_contents)
        .map_err(|e| format!("Failed to write trashinfo {}: {e}", trash_info_path.display()))?;

    move_file(archive_path, &trash_file_path)
}

fn current_iso8601_timestamp() -> String {
    if let Ok(out) = Command::new("date").arg("+%Y-%m-%dT%H:%M:%S").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "1970-01-01T00:00:00".to_string()
}
