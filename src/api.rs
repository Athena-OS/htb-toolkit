use reqwest::blocking::Client;
use core::time::Duration;
use std::io::{self, Write};
use tokio::task;

// Define an async version of fetch_api
pub async fn fetch_api_async(api_url: &str, appkey: &str) -> Result<serde_json::Value, reqwest::Error> {
    let api_url_owned = api_url.to_owned(); // Clone the URL
    let appkey_owned = appkey.to_owned(); // Clone the appkey

    task::spawn_blocking(move || fetch_api(&api_url_owned, &appkey_owned)).await.unwrap()
}

// fetch_api checks also if the App Token is valid or not
fn fetch_api(api_url: &str, appkey: &str) -> Result<serde_json::Value, reqwest::Error> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            // Preserve headers on redirect
            attempt.follow()
        }))
        .build()?;

    let response = client.get(api_url)
        .header("Authorization", format!("Bearer {}", appkey))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("User-Agent", "htb-toolkit")
        .send()?;

    let status_code = response.status().as_u16();
    
    // Check for authentication errors before reading body
    if status_code == 401 {
        eprintln!("\x1B[31mHTTP 401: Unauthenticated. Your API key may be incorrect or expired.\x1B[0m");
        eprintln!("Run 'htb-toolkit -k reset' to set a new key.");
        std::process::exit(1);
    }
    
    let response_text = response.text()?;

    if status_code == 429 {
        eprintln!("HTTP 429: Too many requests. Please wait and try again later.");
        std::process::exit(1);
    } else if (500..600).contains(&status_code) {
        eprintln!("HTTP {status_code}: Server error. Please try again later.");
        std::process::exit(1);
    }

    let json_data: serde_json::Value = match serde_json::from_str(&response_text) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Check if your API key is incorrect or expired. Renew your API key by running 'htb-toolkit -k reset'. Error details: {err}");
            print!("Press Enter to continue...");
            let mut input = String::new();
            io::stdout().flush().expect("Flush failed!");
            io::stdin().read_line(&mut input).expect("Failed to read line");
            std::process::exit(1);
        }
    };

    // HTB retires endpoints with a 200 + {"message": "This endpoint has been removed..."}
    // body. Surface that clearly instead of letting callers fail on a missing field.
    if let Some(msg) = json_data.get("message").and_then(|m| m.as_str()) {
        if msg.contains("endpoint has been removed") {
            eprintln!("\x1B[31mHTB API endpoint no longer exists: {api_url}\x1B[0m");
            eprintln!("Server says: {msg}");
            eprintln!("Please update htb-toolkit or report this at https://github.com/Athena-OS/htb-toolkit/issues");
            std::process::exit(1);
        }
    }

    Ok(json_data)
}