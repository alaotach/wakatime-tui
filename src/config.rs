pub struct Config {
    pub api_key: String,
    pub api_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let file_config = read_wakatime_cfg();

        let api_key = std::env::var("HACKATIME_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| file_config.as_ref().and_then(|cfg| cfg.api_key.clone()))
            .unwrap_or_default();

        let api_url = std::env::var("HACKATIME_API_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| file_config.and_then(|cfg| cfg.api_url))
            .unwrap_or_else(|| "https://hackatime.hackclub.com/api/hackatime/v1".to_string());

        Self { api_key, api_url }
    }
}

struct FileConfig {
    api_key: Option<String>,
    api_url: Option<String>,
}

fn read_wakatime_cfg() -> Option<FileConfig> {
    let home = std::env::var("USERPROFILE")
        .ok()
        .or_else(|| std::env::var("HOME").ok())?;
    let path = format!("{}\\.wakatime.cfg", home);
    let content = std::fs::read_to_string(path).ok()?;

    let mut in_settings = false;
    let mut api_key: Option<String> = None;
    let mut api_url: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_settings = trimmed.eq_ignore_ascii_case("[settings]");
            continue;
        }

        if !in_settings {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            let parsed_key = key.trim();
            let parsed_value = value.trim().to_string();
            if parsed_key.eq_ignore_ascii_case("api_key") {
                api_key = Some(parsed_value);
            } else if parsed_key.eq_ignore_ascii_case("api_url") {
                api_url = Some(parsed_value);
            }
        }
    }

    Some(FileConfig { api_key, api_url })
}