pub struct Config {
    pub api_key: String,
    pub api_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let file_config = read_wakatime_cfg();

        let api_key = first_non_empty_env(&["HACKATIME_API_KEY", "WAKATIME_API_KEY"])
            .or_else(|| file_config.as_ref().and_then(|cfg| cfg.api_key.clone()))
            .unwrap_or_default();

        let api_url = first_non_empty_env(&["HACKATIME_API_URL", "WAKATIME_API_URL"])
            .or_else(|| file_config.and_then(|cfg| cfg.api_url))
            .unwrap_or_else(|| "https://hackatime.hackclub.com/api/hackatime/v1".to_string());

        Self { api_key, api_url }
    }
}

fn first_non_empty_env(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

struct FileConfig {
    api_key: Option<String>,
    api_url: Option<String>,
}

fn read_wakatime_cfg() -> Option<FileConfig> {
    let content = candidate_config_paths()
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())?;

    // Default to parsing top-level keys too; if sections exist, only read [settings].
    let mut in_settings = true;
    let mut saw_section = false;
    let mut api_key: Option<String> = None;
    let mut api_url: Option<String> = None;

    for line in content.lines() {
        // Notepad on Windows may write UTF-8 BOM; strip it before parsing.
        let trimmed = line.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            saw_section = true;
            in_settings = trimmed.eq_ignore_ascii_case("[settings]");
            continue;
        }

        if saw_section && !in_settings {
            continue;
        }

        let Some((key, value)) = trimmed
            .split_once('=')
            .or_else(|| trimmed.split_once(':'))
        else {
            continue;
        };

        let parsed_key = key.trim();
        let parsed_value = value
            .split(['#', ';'])
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        if parsed_value.is_empty() {
            continue;
        }

        if parsed_key.eq_ignore_ascii_case("api_key")
            || parsed_key.eq_ignore_ascii_case("apikey")
        {
                api_key = Some(parsed_value);
        } else if parsed_key.eq_ignore_ascii_case("api_url") {
            api_url = Some(parsed_value);
        }
    }

    Some(FileConfig { api_key, api_url })
}

fn candidate_config_paths() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();

    let mut push_unique = |path: std::path::PathBuf| {
        if !out.iter().any(|p| p == &path) {
            out.push(path);
        }
    };

    // Explicit override takes precedence when provided.
    for key in ["HACKATIME_CONFIG", "WAKATIME_CONFIG"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                push_unique(std::path::PathBuf::from(trimmed));
            }
        }
    }

    if let Ok(value) = std::env::var("WAKATIME_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let base = std::path::Path::new(trimmed);
            push_unique(base.join(".wakatime.cfg"));
            push_unique(base.join("wakatime.cfg"));
        }
    }

    if let Some(home_path) = dirs::home_dir() {
        push_unique(home_path.join(".wakatime.cfg"));
        push_unique(home_path.join("wakatime.cfg"));
        push_unique(home_path.join(".wakatime.cfg.txt"));
        push_unique(home_path.join("wakatime.cfg.txt"));
    }

    // Fallbacks for environments where home_dir cannot be resolved.
    for key in ["USERPROFILE", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                let base = std::path::Path::new(trimmed);
                push_unique(base.join(".wakatime.cfg"));
                push_unique(base.join("wakatime.cfg"));
            }
        }
    }

    let home_drive = std::env::var("HOMEDRIVE").ok().unwrap_or_default();
    let home_path = std::env::var("HOMEPATH").ok().unwrap_or_default();
    if !home_drive.trim().is_empty() && !home_path.trim().is_empty() {
        let combined = format!("{}{}", home_drive.trim(), home_path.trim());
        let base = std::path::Path::new(&combined);
        push_unique(base.join(".wakatime.cfg"));
        push_unique(base.join("wakatime.cfg"));
    }

    if let Ok(appdata) = std::env::var("APPDATA") {
        let trimmed = appdata.trim();
        if !trimmed.is_empty() {
            let appdata_path = std::path::Path::new(trimmed).join("WakaTime");
            push_unique(appdata_path.join(".wakatime.cfg"));
            push_unique(appdata_path.join("wakatime.cfg"));
        }
    }

    out
}