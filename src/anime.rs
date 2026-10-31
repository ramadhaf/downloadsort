use crate::move_file;
use std::fs;
use std::path::{Path, PathBuf};

pub enum AnimeAction {
    UnrecognizedVideo,
    Moved { show: String, dest: PathBuf },
    PlannedMove { show: String, dest: PathBuf },
    SkippedCollision { show: String, dest: PathBuf },
}

pub fn is_video_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mkv" | "mp4" | "avi" | "webm"
    )
}

pub fn extract_anime_show_title(filename: &str) -> Option<String> {
    let path = Path::new(filename);
    if !is_video_file(path) {
        return None;
    }
    let stem = path.file_stem()?.to_str()?.trim();

    if !stem.starts_with('[') {
        return None;
    }
    let group_end = stem.find(']')?;
    let group = stem[1..group_end].trim();
    if group.is_empty() {
        return None;
    }

    let mut rest = stem[group_end + 1..].trim();

    loop {
        let trimmed = rest.trim_end();
        if trimmed.ends_with(']') {
            if let Some(open) = trimmed.rfind('[') {
                rest = trimmed[..open].trim_end();
                continue;
            }
        } else if trimmed.ends_with(')') {
            if let Some(open) = trimmed.rfind('(') {
                rest = trimmed[..open].trim_end();
                continue;
            }
        }
        break;
    }

    for (sep_idx, _) in rest.match_indices(" - ") {
        let title_part = rest[..sep_idx].trim();
        let ep_part = rest[sep_idx + 3..].trim();
        let ep_segment = ep_part.split(" - ").next().unwrap_or("").trim();
        if is_episode_token(ep_segment) {
            if is_safe_show_title(title_part) {
                return Some(title_part.to_string());
            }
            return None;
        }
    }

    None
}

fn is_safe_show_title(title: &str) -> bool {
    !title.is_empty()
        && title != "."
        && title != ".."
        && !title.contains('/')
        && !title.contains('\\')
}

fn is_episode_token(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let first_word = token.split_whitespace().next().unwrap_or("");
    let upper = first_word.to_ascii_uppercase();

    if let Some(after_s) = upper.strip_prefix('S') {
        if let Some((season, ep)) = after_s.split_once('E') {
            if !season.is_empty()
                && season.chars().all(|c| c.is_ascii_digit())
                && !ep.is_empty()
                && ep
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == 'V' || c == '.')
            {
                return true;
            }
        }
    }

    let has_digit = upper.chars().next().is_some_and(|c| c.is_ascii_digit());
    has_digit
        && upper
            .chars()
            .all(|c| c.is_ascii_digit() || c == 'V' || c == '.')
}

pub fn handle_video(
    file_path: &Path,
    anime_root: &Path,
    dry_run: bool,
) -> Result<AnimeAction, String> {
    let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) else {
        return Ok(AnimeAction::UnrecognizedVideo);
    };
    let Some(show) = extract_anime_show_title(filename) else {
        return Ok(AnimeAction::UnrecognizedVideo);
    };

    let show_dir = anime_root.join(&show);
    let dest = show_dir.join(filename);

    if dest.exists() {
        return Ok(AnimeAction::SkippedCollision { show, dest });
    }

    if dry_run {
        return Ok(AnimeAction::PlannedMove { show, dest });
    }

    fs::create_dir_all(&show_dir)
        .map_err(|e| format!("Failed to create show directory {}: {e}", show_dir.display()))?;
    move_file(file_path, &dest)?;

    Ok(AnimeAction::Moved { show, dest })
}
