use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct WallpaperThresholds {
    pub desktop_min_ratio: f64,
    pub mobile_max_ratio: f64,
    pub min_dimension: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub source: String,
    pub anime: String,
    pub desktop_wallpapers: String,
    pub mobile_wallpapers: String,
    pub applications: String,
    pub desktop_launchers: String,
    pub bin_dir: String,
    pub thresholds: WallpaperThresholds,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source: "~/Downloads".to_string(),
            anime: "~/Videos/Anime".to_string(),
            desktop_wallpapers: "~/Pictures/Wallpapers/Desktop".to_string(),
            mobile_wallpapers: "~/Pictures/Wallpapers/Mobile".to_string(),
            applications: "~/Applications".to_string(),
            desktop_launchers: "~/.local/share/applications".to_string(),
            bin_dir: "~/.local/bin".to_string(),
            thresholds: WallpaperThresholds {
                desktop_min_ratio: 1.2,
                mobile_max_ratio: 0.8,
                min_dimension: 1280,
            },
        }
    }
}

impl Config {
    pub fn default_config_path() -> PathBuf {
        expand_tilde("~/.config/downsort/config.json")
    }

    pub fn load_or_init(path: &Path, write_if_missing: bool) -> Result<Self, String> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read config at {}: {e}", path.display()))?;
            Self::from_json(&content)
        } else {
            let cfg = Self::default();
            if write_if_missing {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent).map_err(|e| {
                            format!("Failed to create config directory {}: {e}", parent.display())
                        })?;
                    }
                }
                fs::write(path, cfg.to_json()).map_err(|e| {
                    format!("Failed to write default config to {}: {e}", path.display())
                })?;
            }
            Ok(cfg)
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"source\": \"{}\",\n  \"anime\": \"{}\",\n  \"desktop_wallpapers\": \"{}\",\n  \"mobile_wallpapers\": \"{}\",\n  \"applications\": \"{}\",\n  \"desktop_launchers\": \"{}\",\n  \"bin_dir\": \"{}\",\n  \"desktop_min_ratio\": {},\n  \"mobile_max_ratio\": {},\n  \"min_dimension\": {}\n}}\n",
            escape_json(&self.source),
            escape_json(&self.anime),
            escape_json(&self.desktop_wallpapers),
            escape_json(&self.mobile_wallpapers),
            escape_json(&self.applications),
            escape_json(&self.desktop_launchers),
            escape_json(&self.bin_dir),
            format_float(self.thresholds.desktop_min_ratio),
            format_float(self.thresholds.mobile_max_ratio),
            self.thresholds.min_dimension
        )
    }

    pub fn from_json(input: &str) -> Result<Self, String> {
        let map = parse_flat_json_object(input)?;
        let mut cfg = Self::default();

        if let Some(v) = map.get("source") {
            cfg.source = v.clone();
        }
        if let Some(v) = map.get("anime") {
            cfg.anime = v.clone();
        }
        if let Some(v) = map.get("desktop_wallpapers") {
            cfg.desktop_wallpapers = v.clone();
        }
        if let Some(v) = map.get("mobile_wallpapers") {
            cfg.mobile_wallpapers = v.clone();
        }
        if let Some(v) = map.get("applications") {
            cfg.applications = v.clone();
        }
        if let Some(v) = map.get("desktop_launchers") {
            cfg.desktop_launchers = v.clone();
        }
        if let Some(v) = map.get("bin_dir") {
            cfg.bin_dir = v.clone();
        }
        if let Some(v) = map.get("desktop_min_ratio") {
            cfg.thresholds.desktop_min_ratio = v
                .parse::<f64>()
                .map_err(|e| format!("Invalid desktop_min_ratio '{v}': {e}"))?;
        }
        if let Some(v) = map.get("mobile_max_ratio") {
            cfg.thresholds.mobile_max_ratio = v
                .parse::<f64>()
                .map_err(|e| format!("Invalid mobile_max_ratio '{v}': {e}"))?;
        }
        if let Some(v) = map.get("min_dimension") {
            cfg.thresholds.min_dimension = v
                .parse::<u32>()
                .map_err(|e| format!("Invalid min_dimension '{v}': {e}"))?;
        }

        Ok(cfg)
    }

    pub fn resolve_dir(&self, raw: &str) -> PathBuf {
        expand_tilde(raw)
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn format_float(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') {
        s
    } else {
        format!("{s}.0")
    }
}

fn parse_flat_json_object(input: &str) -> Result<HashMap<String, String>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    skip_ws(&chars, &mut i);
    if i >= chars.len() || chars[i] != '{' {
        return Err("Expected JSON object starting with '{'".to_string());
    }
    i += 1;

    let mut map = HashMap::new();
    loop {
        skip_ws(&chars, &mut i);
        if i >= chars.len() {
            return Err("Unexpected EOF in JSON object".to_string());
        }
        if chars[i] == '}' {
            break;
        }

        let key = parse_json_string(&chars, &mut i)?;
        skip_ws(&chars, &mut i);
        if i >= chars.len() || chars[i] != ':' {
            return Err(format!("Expected ':' after key '{key}'"));
        }
        i += 1;
        skip_ws(&chars, &mut i);
        if i >= chars.len() {
            return Err(format!("Expected value for key '{key}'"));
        }

        let val = if chars[i] == '"' {
            parse_json_string(&chars, &mut i)?
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != ',' && chars[i] != '}'
            {
                i += 1;
            }
            chars[start..i].iter().collect::<String>()
        };

        map.insert(key, val);
        skip_ws(&chars, &mut i);
        if i < chars.len() && chars[i] == ',' {
            i += 1;
        } else if i < chars.len() && chars[i] == '}' {
            break;
        } else {
            return Err("Expected ',' or '}' in JSON object".to_string());
        }
    }

    Ok(map)
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

fn parse_json_string(chars: &[char], i: &mut usize) -> Result<String, String> {
    if *i >= chars.len() || chars[*i] != '"' {
        return Err("Expected '\"' starting JSON string".to_string());
    }
    *i += 1;
    let mut out = String::new();
    while *i < chars.len() {
        let ch = chars[*i];
        *i += 1;
        if ch == '"' {
            return Ok(out);
        }
        if ch == '\\' {
            if *i >= chars.len() {
                return Err("Unexpected EOF in string escape".to_string());
            }
            let esc = chars[*i];
            *i += 1;
            match esc {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
        } else {
            out.push(ch);
        }
    }
    Err("Unterminated JSON string".to_string())
}
