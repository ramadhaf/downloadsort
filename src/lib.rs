pub mod anime;
pub mod app_installer;
pub mod config;
pub mod wallpaper;

use anime::{AnimeAction, handle_video, is_video_file};
use app_installer::{AppAction, handle_archive, is_tar_gz_file};
use config::{Config, escape_json, expand_tilde};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use wallpaper::{WallpaperAction, WallpaperCategory, handle_wallpaper, is_image_file};

#[derive(Default)]
struct SessionOverrides {
    source: Option<String>,
    anime: Option<String>,
    desktop_wallpapers: Option<String>,
    mobile_wallpapers: Option<String>,
    applications: Option<String>,
}

impl SessionOverrides {
    fn apply_to(&self, config: &mut Config) {
        if let Some(v) = &self.source {
            config.source = v.clone();
        }
        if let Some(v) = &self.anime {
            config.anime = v.clone();
        }
        if let Some(v) = &self.desktop_wallpapers {
            config.desktop_wallpapers = v.clone();
        }
        if let Some(v) = &self.mobile_wallpapers {
            config.mobile_wallpapers = v.clone();
        }
        if let Some(v) = &self.applications {
            config.applications = v.clone();
        }
    }
}

pub struct ScanItem {
    pub file: String,
    pub category: String,
    pub status: String,
    pub reason: String,
    pub destination: String,
}

#[derive(Default)]
struct ScanSummary {
    total_scanned: usize,
    anime_moved: usize,
    desktop_wallpapers_moved: usize,
    mobile_wallpapers_moved: usize,
    apps_installed: usize,
    files_skipped: usize,
}

pub fn move_file(src: &Path, dst: &Path) -> Result<(), String> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    fs::copy(src, dst)
        .map_err(|e| format!("Failed to copy {} to {}: {e}", src.display(), dst.display()))?;
    fs::remove_file(src)
        .map_err(|e| format!("Failed to remove source {} after copy: {e}", src.display()))?;
    Ok(())
}

fn require_arg_value(args: &[String], idx: usize, flag: &str) -> Result<String, String> {
    if idx + 1 >= args.len() {
        return Err(format!("{flag} requires a directory path"));
    }
    Ok(args[idx + 1].clone())
}

pub fn run_cli<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let raw_args: Vec<String> = args.into_iter().map(Into::into).collect();
    let mut dry_run = false;
    let mut show_config = false;
    let mut save_config = false;
    let mut json_output = false;
    let mut launch_gui = false;
    let mut config_path: Option<PathBuf> = None;
    let mut overrides = SessionOverrides::default();

    let mut i = 0;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "--show-config" => {
                show_config = true;
                i += 1;
            }
            "--save-config" => {
                save_config = true;
                i += 1;
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            "--gui" => {
                launch_gui = true;
                i += 1;
            }
            "--config" => {
                if i + 1 < raw_args.len() && !raw_args[i + 1].starts_with("--") {
                    config_path = Some(expand_tilde(&raw_args[i + 1]));
                    i += 2;
                } else {
                    show_config = true;
                    i += 1;
                }
            }
            "--source" => {
                overrides.source = Some(require_arg_value(&raw_args, i, "--source")?);
                i += 2;
            }
            "--anime-dir" => {
                overrides.anime = Some(require_arg_value(&raw_args, i, "--anime-dir")?);
                i += 2;
            }
            "--desktop-wallpapers-dir" => {
                overrides.desktop_wallpapers = Some(require_arg_value(
                    &raw_args,
                    i,
                    "--desktop-wallpapers-dir",
                )?);
                i += 2;
            }
            "--mobile-wallpapers-dir" => {
                overrides.mobile_wallpapers = Some(require_arg_value(
                    &raw_args,
                    i,
                    "--mobile-wallpapers-dir",
                )?);
                i += 2;
            }
            "--apps-dir" => {
                overrides.applications = Some(require_arg_value(&raw_args, i, "--apps-dir")?);
                i += 2;
            }
            other => {
                return Err(format!("Unknown argument: {other}"));
            }
        }
    }

    let resolved_config_path = config_path.unwrap_or_else(Config::default_config_path);
    if launch_gui {
        return run_pyqt_gui(&resolved_config_path, &overrides);
    }

    let write_if_missing = !dry_run || show_config;
    let mut config = Config::load_or_init(&resolved_config_path, write_if_missing)?;
    overrides.apply_to(&mut config);

    if save_config {
        config.save(&resolved_config_path)?;
        return Ok(config.to_json());
    }

    if show_config {
        return Ok(config.to_json());
    }

    let source_dir = config.resolve_dir(&config.source);
    if !source_dir.exists() {
        return Err(format!(
            "Source directory does not exist: {}",
            source_dir.display()
        ));
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(&source_dir)
        .map_err(|e| format!("Failed to read source directory {}: {e}", source_dir.display()))?
        .filter_map(|res| res.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    entries.sort();

    let mut summary = ScanSummary::default();
    let mut anime_by_show: BTreeMap<String, usize> = BTreeMap::new();
    let mut logs: Vec<String> = Vec::new();
    let mut items: Vec<ScanItem> = Vec::new();

    let anime_dir = config.resolve_dir(&config.anime);

    for path in &entries {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        summary.total_scanned += 1;
        if is_active_download(path) {
            summary.files_skipped += 1;
            items.push(ScanItem {
                file: name.to_string(),
                category: "Active Download".to_string(),
                status: "Skipped".to_string(),
                reason: "Active Download".to_string(),
                destination: String::new(),
            });
            continue;
        }

        if is_video_file(path) {
            match handle_video(path, &anime_dir, dry_run) {
                Ok(AnimeAction::Moved { show, dest }) => {
                    *anime_by_show.entry(show.clone()).or_insert(0) += 1;
                    logs.push(format!("Anime moved [{show}]: {name} -> {}", dest.display()));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Anime".to_string(),
                        status: "Moved".to_string(),
                        reason: format!("Show: {show}"),
                        destination: dest.display().to_string(),
                    });
                }
                Ok(AnimeAction::PlannedMove { show, dest }) => {
                    *anime_by_show.entry(show.clone()).or_insert(0) += 1;
                    logs.push(format!(
                        "[Dry-Run] Would move anime [{show}]: {name} -> {}",
                        dest.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Anime".to_string(),
                        status: "Will Move".to_string(),
                        reason: format!("Show: {show}"),
                        destination: dest.display().to_string(),
                    });
                }
                Ok(AnimeAction::SkippedCollision { show, dest }) => {
                    summary.files_skipped += 1;
                    logs.push(format!(
                        "Warning: Skipped collision for anime [{show}]: {} already exists",
                        dest.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Anime".to_string(),
                        status: "Skipped".to_string(),
                        reason: "Collision".to_string(),
                        destination: dest.display().to_string(),
                    });
                }
                Ok(AnimeAction::UnrecognizedVideo) => {
                    summary.files_skipped += 1;
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Other".to_string(),
                        status: "Ignored".to_string(),
                        reason: "Non-Anime Video".to_string(),
                        destination: String::new(),
                    });
                }
                Err(err) => {
                    summary.files_skipped += 1;
                    logs.push(format!("Warning: Failed processing {name}: {err}"));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Anime".to_string(),
                        status: "Skipped".to_string(),
                        reason: format!("Error: {err}"),
                        destination: String::new(),
                    });
                }
            }
            continue;
        }

        if is_image_file(path) {
            match handle_wallpaper(path, &config, dry_run) {
                Ok(WallpaperAction::Moved { category, dest }) => {
                    record_wallpaper_category(
                        category,
                        &mut summary.desktop_wallpapers_moved,
                        &mut summary.mobile_wallpapers_moved,
                    );
                    logs.push(format!(
                        "Wallpaper moved [{category:?}]: {name} -> {}",
                        dest.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: wallpaper_category_label(category).to_string(),
                        status: "Moved".to_string(),
                        reason: wallpaper_category_label(category).to_string(),
                        destination: dest.display().to_string(),
                    });
                }
                Ok(WallpaperAction::PlannedMove {
                    category,
                    dest,
                    ratio,
                }) => {
                    record_wallpaper_category(
                        category,
                        &mut summary.desktop_wallpapers_moved,
                        &mut summary.mobile_wallpapers_moved,
                    );
                    logs.push(format!(
                        "[Dry-Run] Would move wallpaper [{category:?}, ratio={ratio:.2}]: {name} -> {}",
                        dest.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: wallpaper_category_label(category).to_string(),
                        status: "Will Move".to_string(),
                        reason: format!("Ratio {ratio:.2}"),
                        destination: dest.display().to_string(),
                    });
                }
                Ok(WallpaperAction::SkippedCollision { category, dest }) => {
                    summary.files_skipped += 1;
                    logs.push(format!(
                        "Warning: Skipped wallpaper collision [{category:?}]: {} already exists",
                        dest.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: wallpaper_category_label(category).to_string(),
                        status: "Skipped".to_string(),
                        reason: "Collision".to_string(),
                        destination: dest.display().to_string(),
                    });
                }
                Ok(WallpaperAction::Ignored) => {
                    summary.files_skipped += 1;
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Other".to_string(),
                        status: "Ignored".to_string(),
                        reason: "Square / Low-Res".to_string(),
                        destination: String::new(),
                    });
                }
                Err(err) => {
                    summary.files_skipped += 1;
                    logs.push(format!("Warning: Failed processing image {name}: {err}"));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Other".to_string(),
                        status: "Skipped".to_string(),
                        reason: format!("Error: {err}"),
                        destination: String::new(),
                    });
                }
            }
            continue;
        }

        if is_tar_gz_file(path) {
            match handle_archive(path, &config, dry_run) {
                Ok(AppAction::Installed(plan)) => {
                    summary.apps_installed += 1;
                    logs.push(format!(
                        "Application installed [{}]: dir={}, bin={}, icon={}, desktop={}",
                        plan.app_name,
                        plan.install_dir.display(),
                        plan.binary.display(),
                        plan.icon,
                        plan.desktop_file.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Application Archive".to_string(),
                        status: "Installed".to_string(),
                        reason: format!("App: {}", plan.app_name),
                        destination: plan.install_dir.display().to_string(),
                    });
                }
                Ok(AppAction::PlannedInstall(plan)) => {
                    summary.apps_installed += 1;
                    logs.push(format!(
                        "[Dry-Run] Would install application [{}]: dir={}, bin={}, icon={}, desktop={}",
                        plan.app_name,
                        plan.install_dir.display(),
                        plan.binary.display(),
                        plan.icon,
                        plan.desktop_file.display()
                    ));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Application Archive".to_string(),
                        status: "Will Install".to_string(),
                        reason: format!("App: {}", plan.app_name),
                        destination: plan.install_dir.display().to_string(),
                    });
                }
                Ok(AppAction::IgnoredNonApp) => {
                    summary.files_skipped += 1;
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Other".to_string(),
                        status: "Ignored".to_string(),
                        reason: "Non-App Archive".to_string(),
                        destination: String::new(),
                    });
                }
                Err(err) => {
                    summary.files_skipped += 1;
                    logs.push(format!("Warning: Failed processing archive {name}: {err}"));
                    items.push(ScanItem {
                        file: name.to_string(),
                        category: "Application Archive".to_string(),
                        status: "Skipped".to_string(),
                        reason: format!("Error: {err}"),
                        destination: String::new(),
                    });
                }
            }
            continue;
        }

        summary.files_skipped += 1;
        items.push(ScanItem {
            file: name.to_string(),
            category: "Other".to_string(),
            status: "Ignored".to_string(),
            reason: "Unrecognized".to_string(),
            destination: String::new(),
        });
    }

    summary.anime_moved = anime_by_show.values().sum();
    if json_output {
        return Ok(format_json_report(dry_run, &summary, &items));
    }

    let mut report = String::new();
    if dry_run {
        report.push_str("=== Dry run mode (no files modified) ===\n");
    }
    for line in &logs {
        report.push_str(line);
        report.push('\n');
    }
    let total_wallpapers = summary.desktop_wallpapers_moved + summary.mobile_wallpapers_moved;
    report.push_str(&format!("Total files scanned: {}\n", summary.total_scanned));
    report.push_str(&format!("Anime videos moved: {}\n", summary.anime_moved));
    for (show, count) in &anime_by_show {
        report.push_str(&format!("  - {show}: {count}\n"));
    }
    report.push_str(&format!(
        "Wallpapers organized: {total_wallpapers} (Desktop: {}, Mobile: {})\n",
        summary.desktop_wallpapers_moved, summary.mobile_wallpapers_moved
    ));
    report.push_str(&format!(
        "Applications installed / updated: {}\n",
        summary.apps_installed
    ));
    report.push_str(&format!("Files skipped: {}\n", summary.files_skipped));

    Ok(report)
}

fn wallpaper_category_label(category: WallpaperCategory) -> &'static str {
    match category {
        WallpaperCategory::Desktop => "Desktop Wallpaper",
        WallpaperCategory::Mobile => "Mobile Wallpaper",
        WallpaperCategory::Ignored => "Other",
    }
}

fn format_json_report(dry_run: bool, summary: &ScanSummary, items: &[ScanItem]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"dry_run\": {dry_run},\n"));
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"total_scanned\": {},\n",
        summary.total_scanned
    ));
    out.push_str(&format!("    \"anime_moved\": {},\n", summary.anime_moved));
    out.push_str(&format!(
        "    \"desktop_wallpapers_moved\": {},\n",
        summary.desktop_wallpapers_moved
    ));
    out.push_str(&format!(
        "    \"mobile_wallpapers_moved\": {},\n",
        summary.mobile_wallpapers_moved
    ));
    out.push_str(&format!(
        "    \"apps_installed\": {},\n",
        summary.apps_installed
    ));
    out.push_str(&format!(
        "    \"files_skipped\": {}\n",
        summary.files_skipped
    ));
    out.push_str("  },\n");
    out.push_str("  \"items\": [\n");
    for (idx, item) in items.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"file\": \"{}\",\n", escape_json(&item.file)));
        out.push_str(&format!(
            "      \"category\": \"{}\",\n",
            escape_json(&item.category)
        ));
        out.push_str(&format!(
            "      \"status\": \"{}\",\n",
            escape_json(&item.status)
        ));
        out.push_str(&format!(
            "      \"reason\": \"{}\",\n",
            escape_json(&item.reason)
        ));
        out.push_str(&format!(
            "      \"destination\": \"{}\"\n",
            escape_json(&item.destination)
        ));
        if idx + 1 < items.len() {
            out.push_str("    },\n");
        } else {
            out.push_str("    }\n");
        }
    }
    out.push_str("  ]\n}\n");
    out
}

fn record_wallpaper_category(
    category: WallpaperCategory,
    desktop_count: &mut usize,
    mobile_count: &mut usize,
) {
    match category {
        WallpaperCategory::Desktop => *desktop_count += 1,
        WallpaperCategory::Mobile => *mobile_count += 1,
        WallpaperCategory::Ignored => {}
    }
}

fn is_active_download(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".crdownload") || lower.ends_with(".part") || lower.ends_with(".tmp")
}

fn run_pyqt_gui(config_path: &Path, overrides: &SessionOverrides) -> Result<String, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Failed to resolve current executable path: {e}"))?;
    let gui_script = include_str!("gui.py");

    let mut cmd = std::process::Command::new("python3");
    cmd.arg("-c")
        .arg(gui_script)
        .arg("--cli-bin")
        .arg(&exe)
        .arg("--config")
        .arg(config_path);

    for (flag, opt_val) in [
        ("--source", &overrides.source),
        ("--anime-dir", &overrides.anime),
        ("--desktop-wallpapers-dir", &overrides.desktop_wallpapers),
        ("--mobile-wallpapers-dir", &overrides.mobile_wallpapers),
        ("--apps-dir", &overrides.applications),
    ] {
        if let Some(v) = opt_val {
            cmd.args([flag, v]);
        }
    }

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to launch python3 for PyQt6 GUI: {e}"))?;
    if !status.success() {
        return Err(format!("PyQt6 GUI exited with status {status}"));
    }
    Ok(String::new())
}
