use reqwest::blocking::Client;
use serde_json::Value;
use std::error::Error;
#[path = "../config.rs"]
mod config;
use config::Config;

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env();
    let token = config.api_key;
    let client = Client::new();
    let projects_url = "https://hackatime.hackclub.com/api/v1/authenticated/projects?include_archived=false";
    
    let resp = client.get(projects_url).bearer_auth(&token).send()?;
    let status = resp.status();
    let body = resp.text()?;

    if !status.is_success() {
        eprintln!("Request failed: {}", status);
        eprintln!("{}", body);
        return Ok(());
    }

    let respjson: Value = match serde_json::from_str(&body) {
        Ok(parsed) => parsed,
        Err(_) => {
            println!("{}", body);
            return Ok(());
        }
    };
    println!("{}", serde_json::to_string_pretty(&respjson)?);
    Ok(())
}