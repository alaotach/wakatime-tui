use serde::Deserialize;
use reqwest::Client;
use crate::config::Config;

#[derive(Debug, Deserialize)]
pub struct SummaryResponse {
    pub data: TodayData,
}

#[derive(Debug, Deserialize)]
pub struct TodayData {
    pub grand_total: GrandTotal,
    #[serde(default)]
    pub goal: Goal,
}

#[derive(Debug, Deserialize)]
pub struct GrandTotal {
    pub text: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Goal {
    #[serde(default)]
    pub completion_percent: u64,
}

pub async fn fetch_summary(config: &Config) -> Result<SummaryResponse, String> {
    let client = Client::new();
    let base = config.api_url.trim_end_matches('/');
    let endpoint = format!("{}/users/current/statusbar/today", base);

    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|error| format!("request failed: {}", error))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|error| format!("failed reading response body: {}", error))?;
    let preview: String = body.chars().collect();

    serde_json::from_str::<SummaryResponse>(&body).map_err(|error| format!("error response: {}; {}; status: {}", error, preview, status))
}