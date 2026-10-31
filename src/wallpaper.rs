use crate::config::{Config, WallpaperThresholds};
use crate::move_file;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperCategory {
    Desktop,
    Mobile,
    Ignored,
}

pub enum WallpaperAction {
    Ignored,
    Moved {
        category: WallpaperCategory,
        dest: PathBuf,
    },
    PlannedMove {
        category: WallpaperCategory,
        dest: PathBuf,
        ratio: f64,
    },
    SkippedCollision {
        category: WallpaperCategory,
        dest: PathBuf,
    },
}

pub fn is_image_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp"
    )
}

pub fn classify_dimensions(
    width: u32,
    height: u32,
    thresholds: &WallpaperThresholds,
) -> WallpaperCategory {
    if width == 0 || height == 0 {
        return WallpaperCategory::Ignored;
    }
    if width.max(height) < thresholds.min_dimension {
        return WallpaperCategory::Ignored;
    }
    let ratio = (width as f64) / (height as f64);
    if ratio > thresholds.desktop_min_ratio && width >= thresholds.min_dimension {
        WallpaperCategory::Desktop
    } else if ratio < thresholds.mobile_max_ratio && height >= thresholds.min_dimension {
        WallpaperCategory::Mobile
    } else {
        WallpaperCategory::Ignored
    }
}

pub fn read_image_dimensions(path: &Path) -> Result<(u32, u32), String> {
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open image {}: {e}", path.display()))?;
    let mut header = [0u8; 32];
    let n = file
        .read(&mut header)
        .map_err(|e| format!("Failed to read image header {}: {e}", path.display()))?;

    if n >= 24
        && header[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        && &header[12..16] == b"IHDR"
    {
        let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
        let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
        if width == 0 || height == 0 {
            return Err("PNG has zero width or height".to_string());
        }
        return Ok((width, height));
    }

    if n >= 30 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        let chunk = &header[12..16];
        if chunk == b"VP8X" {
            let w_minus_1 = (header[24] as u32)
                | ((header[25] as u32) << 8)
                | ((header[26] as u32) << 16);
            let h_minus_1 = (header[27] as u32)
                | ((header[28] as u32) << 8)
                | ((header[29] as u32) << 16);
            return Ok((w_minus_1 + 1, h_minus_1 + 1));
        } else if chunk == b"VP8L" && header[20] == 0x2F {
            let b0 = header[21] as u32;
            let b1 = header[22] as u32;
            let b2 = header[23] as u32;
            let b3 = header[24] as u32;
            let width = (b0 | ((b1 & 0x3F) << 8)) + 1;
            let height = ((b1 >> 6) | (b2 << 2) | ((b3 & 0x0F) << 10)) + 1;
            return Ok((width, height));
        } else if chunk == b"VP8 " && header[23..26] == [0x9D, 0x01, 0x2A] {
            let width = (u16::from_le_bytes([header[26], header[27]]) & 0x3FFF) as u32;
            let height = (u16::from_le_bytes([header[28], header[29]]) & 0x3FFF) as u32;
            if width > 0 && height > 0 {
                return Ok((width, height));
            }
        }
    }

    if n >= 4 && header[0] == 0xFF && header[1] == 0xD8 {
        return read_jpeg_dimensions(&mut file);
    }

    Err(format!(
        "Unrecognized or corrupted image header in {}",
        path.display()
    ))
}

fn read_jpeg_dimensions(file: &mut File) -> Result<(u32, u32), String> {
    file.seek(SeekFrom::Start(2))
        .map_err(|e| format!("JPEG seek error: {e}"))?;
    let mut buf = [0u8; 9];

    loop {
        let mut b = [0u8; 1];
        file.read_exact(&mut b)
            .map_err(|e| format!("Corrupt or truncated JPEG stream: {e}"))?;
        if b[0] != 0xFF {
            continue;
        }
        let marker = loop {
            file.read_exact(&mut b)
                .map_err(|e| format!("Unexpected EOF reading JPEG marker: {e}"))?;
            if b[0] != 0xFF {
                break b[0];
            }
        };

        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if marker == 0x00 || (0xD0..=0xD8).contains(&marker) || marker == 0x01 {
            continue;
        }

        let mut len_bytes = [0u8; 2];
        file.read_exact(&mut len_bytes)
            .map_err(|e| format!("Unexpected EOF reading JPEG segment length: {e}"))?;
        let seg_len = u16::from_be_bytes(len_bytes) as usize;
        if seg_len < 2 {
            return Err("Invalid JPEG segment length".to_string());
        }

        let is_sof = matches!(
            marker,
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
        );
        if is_sof {
            if seg_len < 7 {
                return Err("Truncated JPEG SOF segment".to_string());
            }
            file.read_exact(&mut buf[..5])
                .map_err(|e| format!("Truncated JPEG SOF dimensions: {e}"))?;
            let height = u16::from_be_bytes([buf[1], buf[2]]) as u32;
            let width = u16::from_be_bytes([buf[3], buf[4]]) as u32;
            if width == 0 || height == 0 {
                return Err("JPEG has zero width or height".to_string());
            }
            return Ok((width, height));
        }

        file.seek(SeekFrom::Current((seg_len - 2) as i64))
            .map_err(|e| format!("JPEG segment seek error: {e}"))?;
    }

    Err("No SOF dimension marker found in JPEG".to_string())
}

pub fn handle_wallpaper(
    file_path: &Path,
    config: &Config,
    dry_run: bool,
) -> Result<WallpaperAction, String> {
    let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) else {
        return Ok(WallpaperAction::Ignored);
    };

    let (width, height) = read_image_dimensions(file_path)?;
    let category = classify_dimensions(width, height, &config.thresholds);

    let target_dir = match category {
        WallpaperCategory::Desktop => config.resolve_dir(&config.desktop_wallpapers),
        WallpaperCategory::Mobile => config.resolve_dir(&config.mobile_wallpapers),
        WallpaperCategory::Ignored => return Ok(WallpaperAction::Ignored),
    };

    let dest = target_dir.join(filename);
    if dest.exists() {
        return Ok(WallpaperAction::SkippedCollision { category, dest });
    }

    let ratio = (width as f64) / (height as f64);
    if dry_run {
        return Ok(WallpaperAction::PlannedMove {
            category,
            dest,
            ratio,
        });
    }

    fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "Failed to create wallpaper directory {}: {e}",
            target_dir.display()
        )
    })?;
    move_file(file_path, &dest)?;

    Ok(WallpaperAction::Moved { category, dest })
}
