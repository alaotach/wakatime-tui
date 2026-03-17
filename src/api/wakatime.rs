use serde::Deserialize;
use reqwest::Client;
use crate::config::Config;
use chrono::{Utc, TimeZone, NaiveDate, Local, Datelike};
use std::{collections::BTreeMap, fs};
use std::path::Path;

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

#[derive(Debug, Deserialize)]
pub struct WeeklyStatsResponse {
    pub data: WeeklyData,
}

#[derive(Debug, Deserialize)]
pub struct UserResponse {
    pub data: UserData,
}

#[derive(Debug, Deserialize)]
pub struct UserData {
    pub username: String,
}

#[derive(Debug, Deserialize)]
struct HeartbeatsFile {
    spans: Vec<Span>,
}

#[derive(Debug, Deserialize)]
struct SpansDataResponse {
    data: Vec<Span>,
}

#[derive(Debug, Deserialize)]
struct Span {
    #[serde(alias = "start_time")]
    start: f64,
    #[serde(alias = "end_time")]
    end: f64,
}

#[derive(Debug, Deserialize)]
pub struct WeeklyData {
    pub human_readable_range: String,
    pub human_readable_total: String,
    #[serde(default)]
    pub languages: Vec<Item>,
    #[serde(default)]
    pub projects: Vec<Item>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Item {
    pub name: String,
    pub text: String,
    pub percent: f64,
}

pub async fn get_username(config: &Config) -> Result<String, String> {
    if let Ok(username) = std::env::var("WAKATIME_USERNAME") {
        return Ok(username);
    }
    let client = Client::new();
    let base = config.api_url.trim_end_matches('/');
    let endpoint = format!("{}/users/current/stats/last_7_days", base);

    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|error| format!("request failed: {}", error))?;

    let body = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: UserResponse =
        serde_json::from_str(&body).map_err(|e| format!("json error: {}", e))?;
    unsafe{
        std::env::set_var("WAKATIME_USERNAME", &parsed.data.username);
    }
    Ok(parsed.data.username)
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

pub async fn fetch_weekly_stats(config: &Config) -> Result<WeeklyStatsResponse, String> {
    let client = Client::new();
    let base = config.api_url.trim_end_matches('/');
    let endpoint = format!("{}/users/current/stats/last_7_days", base);
    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|error| format!("request failed: {}", error))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|error| format!("failed reading response body: {}", error))?;
    let preview: String = body.chars().collect();
    serde_json::from_str::<WeeklyStatsResponse>(&body).map_err(|error| format!("error response: {}; {}; status: {}", error, preview, status))
}

pub async fn fetch_spans(config: &Config) -> Result<BTreeMap<NaiveDate, f64>, String> {
    let home = dirs::home_dir().ok_or("not found")?;
    let path = home.join(".heartbeats.json");
    let mut cached: BTreeMap<NaiveDate, f64> = if path.exists() {
        let raw = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let username = get_username(config).await?;
    let client = Client::new();
    let base = config.api_url.trim_end_matches('/');
    let endpoint = format!("https://hackatime.hackclub.com/api/v1/users/{}/heartbeats/spans", username);
    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let mut spans = parse_spans_payload(&body)?;
    if spans.is_empty() && Path::new("heartbeats.json").exists() {
        let local = fs::read_to_string("heartbeats.json").unwrap_or_default();
        spans = parse_spans_payload(&local)?;
    }
    let now = Utc::now().timestamp() as f64;
    let cutoff = now - 360.0*24.0*3600.0;
    let mut fresh: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for s in spans {
        let mut a = s.start.max(cutoff);
        let b = s.end.min(now);
        if b <= a { continue; }
        while a < b {
            let dt = Local.timestamp_opt(a as i64, 0).unwrap();
            let day = dt.date_naive();
            let next_midnight = Local.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 0, 0, 0, ).unwrap().checked_add_signed(chrono::Duration::days(1)).unwrap().timestamp() as f64;
            let chunk_end = b.min(next_midnight);
            *fresh.entry(day).or_insert(0.0) += chunk_end - a;
            a = chunk_end;
        }
    }
    for (k, v) in fresh {
        cached.insert(k, v);
    }
    if let Ok(json) = serde_json::to_string_pretty(&cached) {
        let _ = fs::write(&path, json);
    }
    Ok(cached)
}

fn parse_spans_payload(raw: &str) -> Result<Vec<Span>, String> {
    if let Ok(spans) = serde_json::from_str::<Vec<Span>>(raw) {
        Ok(spans)
    } else if let Ok(wrapped) = serde_json::from_str::<HeartbeatsFile>(raw) {
        Ok(wrapped.spans)
    } else if let Ok(wrapped) = serde_json::from_str::<SpansDataResponse>(raw) {
        Ok(wrapped.data)
    } else {
        Err("unable to parse heartbeat spans payload".to_string())
    }
}