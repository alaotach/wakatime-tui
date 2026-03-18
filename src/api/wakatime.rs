use serde::Deserialize;
use reqwest::Client;
use crate::config::Config;
use chrono::{Utc, TimeZone, NaiveDate, Local, Datelike, Timelike};
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

#[derive(Debug, Deserialize, Clone)]
pub struct ProjectDetail {
    pub name: String,
    pub total_seconds: u64,
    pub total_heartbeats: u64,
    pub archived: bool,
    #[serde(default)]
    pub languages: Vec<String>,
}
 
#[derive(Deserialize)]
struct ProjectsResponse {
    projects: Vec<ProjectDetail>,
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

pub async fn fetch_projects(config: &Config) -> Result<Vec<ProjectDetail>, String> {
    let username = get_username(config).await?;
    let client = Client::new();
    let endpoint = format!( "https://hackatime.hackclub.com/api/v1/users/{}/projects/details?since=25-12-2007",
        username
    );
    let resp = client
        .get(&endpoint)
        .bearer_auth(&config.api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: ProjectsResponse = serde_json::from_str(&body)
        .map_err(|e| format!("parse error: {}: {}", e, &body[..body.len().min(200)]))?;
    let mut projects = parsed.projects;
    projects.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));
    Ok(projects)
}

pub async fn fetch_daily_leaderboard(config: &Config) -> Result<Vec<(String, String)>, String> {
    let client = Client::new();
    let endpoint = "https://hackatime.hackclub.com/api/v1/leaderboard/daily".to_string();
    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("parse error: {}: {}", e, &body[..body.len().min(200)]))?;
    Ok(parsed.get("entries").and_then(|d| d.as_array()).unwrap_or(&vec![]).iter().map(|entry| {
        let username = entry.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()).unwrap_or("unknown").to_string();
        let secs = entry.get("total_seconds").and_then(|s| s.as_u64()).unwrap_or(0);
        (username, format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60))
    }).collect())
}

pub async fn fetch_weekly_leaderboard(config: &Config) -> Result<Vec<(String, String)>, String> {
    let client = Client::new();
    let endpoint = "https://hackatime.hackclub.com/api/v1/leaderboard/weekly".to_string();
    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("parse error: {}: {}", e, &body[..body.len().min(200)]))?;
    Ok(parsed.get("entries").and_then(|d| d.as_array()).unwrap_or(&vec![]).iter().map(|entry| {
        let username = entry.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()).unwrap_or("unknown").to_string();
        let secs = entry.get("total_seconds").and_then(|s| s.as_u64()).unwrap_or(0);
        (username, format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60))
    }).collect())
}

pub fn streaks(activity: &BTreeMap<NaiveDate, f64>) -> (u64, u64) {
    let today = Local::now().date_naive();
    let min = 60.0;
    let mut days: Vec<NaiveDate> = activity
        .iter()
        .filter(|(_, v)| **v >= min)
        .map(|(d, _)| *d)
        .collect();
    days.sort();
    if days.is_empty() {
        return (0, 0);
    }
    let mut current = 0u32;
    let mut check = if activity.get(&today).copied().unwrap_or(0.0) >= min {
        today
    } else {
        today - chrono::Duration::days(1)
    };
    loop {
        if activity.get(&check).copied().unwrap_or(0.0) >= min {
            current += 1;
            check -= chrono::Duration::days(1);
        } else {
            break;
        }
    }
    let mut longest = 0u32;
    let mut run = 1u32;
    for i in 1..days.len() {
        let diff = (days[i] - days[i-1]).num_days();
        if diff == 1 {
            run += 1;
            if run > longest {
                longest = run;
            }
        } else {
            run = 1;
        }
    }
    longest = longest.max(run);
    (current as u64, longest as u64)
}

pub async fn fetch_hourly(config: &Config, date: NaiveDate) -> Result<[f64; 24], String> {
    let username = get_username(config).await?;
    let client = Client::new();
    let qstart = date - chrono::Duration::days(1);
    let qend = date + chrono::Duration::days(1);
    let endpoint = format!(
        "https://hackatime.hackclub.com/api/v1/users/{}/heartbeats/spans?start_date={}&end_date={}", username, qstart.format("%d-%m-%Y"), qend.format("%d-%m-%Y")
    );

    let resp = client.get(&endpoint).bearer_auth(&config.api_key).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let spans = parse_spans_payload(&body)?;
    let day_start = Local.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0).unwrap().timestamp() as f64;
    let next_day = date + chrono::Duration::days(1);
    let day_end = Local.with_ymd_and_hms(next_day.year(), next_day.month(), next_day.day(), 0, 0, 0).unwrap().timestamp() as f64;

    let mut hours = [0f64; 24];
    for s in spans {
        let a = s.start.max(day_start);
        let b = s.end.min(day_end);
        if b <= a {
            continue;
        }
        let mut cur = a;
        while cur < b {
            let dt = Local.timestamp_opt(cur as i64, 0).unwrap();
            let hour = dt.hour() as usize;
            let next_hr = Local.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), hour as u32, 0, 0).unwrap().checked_add_signed(chrono::Duration::hours(1)).unwrap().timestamp() as f64;
            let chunk_end = b.min(next_hr);
            hours[hour] += chunk_end - cur;
            cur = chunk_end;
        }
    }
    Ok(hours)
}