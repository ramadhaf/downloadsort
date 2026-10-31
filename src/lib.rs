pub mod anime;
pub mod app_installer;
pub mod config;
pub mod wallpaper;

use anime::{AnimeAction, handle_video, is_video_file};
use app_installer::{AppAction, handle_archive, is_tar_gz_file};
use config::{Config, expand_tilde};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use wallpaper::{WallpaperAction, WallpaperCategory, handle_wallpaper, is_image_file};

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

pub fn run_cli<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let raw_args: Vec<String> = args.into_iter().map(Into::into).collect();
    let mut dry_run = false;
    let mut show_config = false;
    let mut config_path: Option<PathBuf> = None;
    let mut source_override: Option<String> = None;

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
                if i + 1 >= raw_args.len() {
                    return Err("--source requires a directory path".to_string());
                }
                source_override = Some(raw_args[i + 1].clone());
                i += 2;
            }
            other => {
                return Err(format!("Unknown argument: {other}"));
            }
        }
    }

    let resolved_config_path = config_path.unwrap_or_else(Config::default_config_path);
    let write_if_missing = !dry_run || show_config;
    let mut config = Config::load_or_init(&resolved_config_path, write_if_missing)?;
    if let Some(src) = source_override {
        config.source = src;
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

    let mut total_scanned = 0usize;
    let mut skipped_count = 0usize;
    let mut anime_by_show: BTreeMap<String, usize> = BTreeMap::new();
    let mut desktop_wallpapers_moved = 0usize;
    let mut mobile_wallpapers_moved = 0usize;
    let mut apps_installed = 0usize;
    let mut logs: Vec<String> = Vec::new();

    let anime_dir = config.resolve_dir(&config.anime);

    for path in &entries {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        total_scanned += 1;
        if is_active_download(path) {
            skipped_count += 1;
            continue;
        }

        if is_video_file(path) {
            match handle_video(path, &anime_dir, dry_run) {
                Ok(AnimeAction::Moved { show, dest }) => {
                    *anime_by_show.entry(show.clone()).or_insert(0) += 1;
                    logs.push(format!("Anime moved [{show}]: {name} -> {}", dest.display()));
                }
                Ok(AnimeAction::PlannedMove { show, dest }) => {
                    *anime_by_show.entry(show.clone()).or_insert(0) += 1;
                    logs.push(format!(
                        "[Dry-Run] Would move anime [{show}]: {name} -> {}",
                        dest.display()
                    ));
                }
                Ok(AnimeAction::SkippedCollision { show, dest }) => {
                    skipped_count += 1;
                    logs.push(format!(
                        "Warning: Skipped collision for anime [{show}]: {} already exists",
                        dest.display()
                    ));
                }
                Ok(AnimeAction::UnrecognizedVideo) => {
                    skipped_count += 1;
                }
                Err(err) => {
                    skipped_count += 1;
                    logs.push(format!("Warning: Failed processing {name}: {err}"));
                }
            }
            continue;
        }

        if is_image_file(path) {
            match handle_wallpaper(path, &config, dry_run) {
                Ok(WallpaperAction::Moved { category, dest }) => {
                    record_wallpaper_category(
                        category,
                        &mut desktop_wallpapers_moved,
                        &mut mobile_wallpapers_moved,
                    );
                    logs.push(format!(
                        "Wallpaper moved [{category:?}]: {name} -> {}",
                        dest.display()
                    ));
                }
                Ok(WallpaperAction::PlannedMove {
                    category,
                    dest,
                    ratio,
                }) => {
                    record_wallpaper_category(
                        category,
                        &mut desktop_wallpapers_moved,
                        &mut mobile_wallpapers_moved,
                    );
                    logs.push(format!(
                        "[Dry-Run] Would move wallpaper [{category:?}, ratio={ratio:.2}]: {name} -> {}",
                        dest.display()
                    ));
                }
                Ok(WallpaperAction::SkippedCollision { category, dest }) => {
                    skipped_count += 1;
                    logs.push(format!(
                        "Warning: Skipped wallpaper collision [{category:?}]: {} already exists",
                        dest.display()
                    ));
                }
                Ok(WallpaperAction::Ignored) => {
                    skipped_count += 1;
                }
                Err(err) => {
                    skipped_count += 1;
                    logs.push(format!("Warning: Failed processing image {name}: {err}"));
                }
            }
            continue;
        }

        if is_tar_gz_file(path) {
            match handle_archive(path, &config, dry_run) {
                Ok(AppAction::Installed {
                    app_name,
                    install_dir,
                    binary,
                    icon,
                    desktop_file,
                }) => {
                    apps_installed += 1;
                    logs.push(format!(
                        "Application installed [{app_name}]: dir={}, bin={}, icon={icon}, desktop={}",
                        install_dir.display(),
                        binary.display(),
                        desktop_file.display()
                    ));
                }
                Ok(AppAction::PlannedInstall {
                    app_name,
                    install_dir,
                    binary,
                    icon,
                    desktop_file,
                }) => {
                    apps_installed += 1;
                    logs.push(format!(
                        "[Dry-Run] Would install application [{app_name}]: dir={}, bin={}, icon={icon}, desktop={}",
                        install_dir.display(),
                        binary.display(),
                        desktop_file.display()
                    ));
                }
                Ok(AppAction::IgnoredNonApp) => {
                    skipped_count += 1;
                }
                Err(err) => {
                    skipped_count += 1;
                    logs.push(format!("Warning: Failed processing archive {name}: {err}"));
                }
            }
            continue;
        }

        skipped_count += 1;
    }

    let mut report = String::new();
    if dry_run {
        report.push_str("=== Dry run mode (no files modified) ===\n");
    }
    for line in &logs {
        report.push_str(line);
        report.push('\n');
    }
    let anime_total: usize = anime_by_show.values().sum();
    let total_wallpapers = desktop_wallpapers_moved + mobile_wallpapers_moved;
    report.push_str(&format!("Total files scanned: {total_scanned}\n"));
    report.push_str(&format!("Anime videos moved: {anime_total}\n"));
    for (show, count) in &anime_by_show {
        report.push_str(&format!("  - {show}: {count}\n"));
    }
    report.push_str(&format!(
        "Wallpapers organized: {total_wallpapers} (Desktop: {desktop_wallpapers_moved}, Mobile: {mobile_wallpapers_moved})\n"
    ));
    report.push_str(&format!("Applications installed / updated: {apps_installed}\n"));
    report.push_str(&format!("Files skipped: {skipped_count}\n"));

    Ok(report)
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
