use crate::api::fetch_api_async;
use crate::appkey::get_appkey;
use crate::colors::*;
use crate::utils::*;
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;
use reqwest::Client;
use tokio::spawn;

/// Which HTB VPN product to connect to. The string returned by
/// `api_type()` must match the `type` field of `/api/v5/connections`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnKind {
    Labs,
    StartingPoint,
    Fortresses,
}

impl VpnKind {
    pub fn api_type(self) -> &'static str {
        match self {
            VpnKind::Labs => "labs",
            VpnKind::StartingPoint => "starting_point",
            VpnKind::Fortresses => "fortresses",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            VpnKind::Labs => "Machines",
            VpnKind::StartingPoint => "Starting Point",
            VpnKind::Fortresses => "Fortress",
        }
    }

    pub fn from_sp_flag(is_starting_point: bool) -> Self {
        if is_starting_point {
            VpnKind::StartingPoint
        } else {
            VpnKind::Labs
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct VpnServer {
    id: i64,
    friendly_name: String,
    current_clients: i64,
    location: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct VpnAccess {
    access_type: String,
    /// e.g. "EU - Free", "US - VIP+". Null for products the account can't use.
    location_type: Option<String>,
    can_access: bool,
    server: Option<VpnServer>,
}

/// Parse the body of `GET /api/v5/connections`.
///
/// `data` is an array of entries discriminated by `type`
/// (labs | starting_point | fortresses | prolab | competitive).
/// Any field other than `type` may be null or absent (e.g. a ProLab the user
/// doesn't own has `location_type_friendly: null, assigned_server: null`),
/// so a single odd entry must never abort the whole list.
fn parse_connections(json_value: &Value) -> Result<Vec<VpnAccess>, String> {
    // The v4 endpoint was retired and now answers {"message": "This endpoint has been removed..."}
    if let Some(msg) = json_value.get("message").and_then(|m| m.as_str()) {
        if json_value.get("data").is_none() {
            return Err(format!("API said: {msg}"));
        }
    }

    let data = json_value
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing or invalid `data` array in /api/v5/connections response".to_string())?;

    let mut connections = Vec::new();

    for entry in data {
        let access_type = match entry.get("type").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => continue, // skip malformed entries
        };

        // `available` only appears on some entries (e.g. competitive).
        let can_access = entry
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let server = entry
            .get("assigned_server")
            .filter(|s| !s.is_null())
            .map(|s| VpnServer {
                id: s.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                friendly_name: s
                    .get("friendly_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                current_clients: s.get("current_clients").and_then(|v| v.as_i64()).unwrap_or(0),
                location: s
                    .get("location")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
            })
            // an assigned server without an id is useless (we need it for the ovpn download)
            .filter(|s| s.id > 0);

        let location_type = entry
            .get("location_type_friendly")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        connections.push(VpnAccess {
            access_type,
            location_type,
            can_access,
            server,
        });
    }

    Ok(connections)
}

async fn get_vpn_connections() -> Result<Vec<VpnAccess>, String> {
    let appkey = get_appkey();
    let result = fetch_api_async("https://labs.hackthebox.com/api/v5/connections", &appkey).await;

    match result {
        Ok(json_value) => parse_connections(&json_value),
        Err(err) => Err(format!("API error: {:?}", err)),
    }
}

async fn vpn_type() -> Option<Vec<String>> {
    let appkey = get_appkey();
    let result = fetch_api_async("https://labs.hackthebox.com/api/v4/connection/status", &appkey);
    let mut vpntype: Vec<String> = Vec::new();

    match result.await {
        Ok(json_value) => {
            // Historically a bare array; tolerate a {"data": [...]} wrapper too.
            let arr = json_value
                .as_array()
                .or_else(|| json_value.get("data").and_then(|d| d.as_array()));
            if let Some(json_vpn) = arr {
                for item in json_vpn {
                    if let Some(vpntype_value) = item["type"].as_str() {
                        vpntype.push(vpntype_value.to_string());
                    }
                }
            }
        }
        Err(err) => {
            if err.is_timeout() {
                eprintln!("Encountered timeout");
            } else {
                eprintln!(
                    "\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m"
                );
            }
        }
    }

    if get_vpn_interface_ip().is_some() {
        Some(vpntype)
    } else {
        None
    }
}

pub async fn check_vpn(machine_spflag: bool) {
    let kind = VpnKind::from_sp_flag(machine_spflag);

    if let Some(vpntypes) = vpn_type().await {
        let vpntypes_str = if vpntypes.is_empty() {
            "unknown (a tun/wg interface is up but HTB reports no connection)".to_string()
        } else {
            vpntypes.join(", ")
        };
        let mut yn = String::new();

        if vpntypes.len() > 1 {
            println!(
                "\nThe following VPN types are already running: {vpntypes_str}. You have multiple VPNs running. The oldest one will go down automatically in some minutes."
            );
        } else {
            println!("\nThe following VPN type is already running: {vpntypes_str}.");
        }

        print!("Do you want to terminate the listed VPN and connect to a different one (y/n)? ");
        io::stdout().flush().expect("Flush failed!");
        io::stdin().read_line(&mut yn).expect("Failed to read input");

        if matches!(yn.trim(), "y" | "Y") {
            run_vpn(kind).await;
        }
    } else {
        run_vpn(kind).await;
    }
}

pub async fn run_vpn(kind: VpnKind) {
    let appkey = get_appkey();

    // Get available connections
    let connections = match get_vpn_connections().await {
        Ok(conns) => conns,
        Err(e) => {
            eprintln!("\x1B[31mFailed to get VPN connections: {}\x1B[0m", e);
            std::process::exit(1);
        }
    };

    // Keep only entries of the requested product that actually have a server
    // we can download a config for.
    let filtered: Vec<VpnAccess> = connections
        .into_iter()
        .filter(|c| c.access_type == kind.api_type() && c.can_access && c.server.is_some())
        .collect();

    if filtered.is_empty() {
        eprintln!(
            "\x1B[31mNo {} VPN server is assigned to your account.\x1B[0m",
            kind.label()
        );
        eprintln!(
            "Open https://app.hackthebox.com/ -> Connect to HTB -> {} and pick a region once; HTB will then report it on /api/v5/connections.",
            kind.label()
        );
        std::process::exit(1);
    }

    // Display available connections
    println!("\n{BCYAN}Available {} VPN Connections:{RESET}", kind.label());
    for (idx, conn) in filtered.iter().enumerate() {
        let server = conn.server.as_ref().expect("filtered above");
        println!(
            "{}. {} - {} ({} users)",
            idx + 1,
            conn.location_type.as_deref().unwrap_or(&server.location),
            server.friendly_name,
            server.current_clients
        );
    }

    // Select connection
    let selected = if filtered.len() == 1 {
        println!("\n{BGREEN}Auto-selecting the only available server.{RESET}");
        filtered[0].clone()
    } else {
        print!("\nSelect VPN (1-{}): ", filtered.len());
        io::stdout().flush().expect("Flush failed!");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");

        let choice: usize = input.trim().parse().unwrap_or(1);
        if choice < 1 || choice > filtered.len() {
            eprintln!("Invalid selection");
            std::process::exit(1);
        }
        filtered[choice - 1].clone()
    };

    let server = selected.server.expect("filtered above");

    // Ask for TCP/UDP
    let mut use_tcp = false;
    loop {
        let mut input = String::new();
        print!("\n{BGREEN}Would you like to connect to Hack The Box VPN by UDP or TCP? [UDP] {RESET}");
        io::stdout().flush().expect("Flush failed!");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "" | "udp" => break,
            "tcp" => {
                use_tcp = true;
                break;
            }
            _ => println!("{BGREEN}Please select UDP or TCP:{RESET}"),
        }
    }

    println!("\nConnecting to {} [id={}]\n", server.friendly_name, server.id);

    // Kill existing OpenVPN
    let _output = Command::new("sudo")
        .arg("killall")
        .arg("openvpn")
        .output()
        .expect("Failed to execute command");

    // Download and start VPN
    let started = spawn(async move {
        let client = Client::new();

        // UDP: /access/ovpnfile/{id}/0   TCP: /access/ovpnfile/{id}/0/1
        let ovpn_url = format!(
            "https://labs.hackthebox.com/api/v4/access/ovpnfile/{}/0{}",
            server.id,
            if use_tcp { "/1" } else { "" }
        );

        let ovpn_response = client
            .get(ovpn_url)
            .header("Authorization", format!("Bearer {}", appkey))
            .send()
            .await;

        let response = match ovpn_response {
            Ok(r) => r,
            Err(err) => {
                eprintln!("\x1B[31mAPI call error: {err:?}\x1B[0m");
                return false;
            }
        };

        if !response.status().is_success() {
            eprintln!("\x1B[31mAPI call failed with status: {}\x1B[0m", response.status());
            return false;
        }

        let ovpn_content = match response.text().await {
            Ok(t) => t,
            Err(err) => {
                eprintln!("\x1B[31mFailed to read VPN config: {err}\x1B[0m");
                return false;
            }
        };

        // The API answers 200 with a JSON error body in some failure cases.
        if !ovpn_content.contains("client") || ovpn_content.trim_start().starts_with('{') {
            eprintln!("\x1B[31mUnexpected VPN config content:\x1B[0m {}", ovpn_content.trim());
            return false;
        }

        let ovpn_file_path = format!(
            "{}/lab-vpn.ovpn",
            std::env::var("HOME").unwrap_or_default()
        );

        if let Err(err) = fs::write(&ovpn_file_path, ovpn_content) {
            eprintln!("Error writing to file: {err}");
            return false;
        }
        println!("VPN config file saved successfully.");

        let status = Command::new("sudo")
            .arg("openvpn")
            .arg("--config")
            .arg(&ovpn_file_path)
            .arg("--daemon")
            .status()
            .expect("Failed to execute openvpn command");

        if !status.success() {
            eprintln!("\x1B[31mOpenVPN process exited with error: {status:?}\x1B[0m");
            return false;
        }

        println!("{BGREEN}OpenVPN started successfully{RESET}");
        true
    })
    .await
    .expect("Blocking task failed");

    if !started {
        std::process::exit(1);
    }

    // `openvpn --daemon` returns immediately; wait for the tunnel interface
    // to actually get an address before letting callers read it.
    print!("Waiting for the VPN tunnel to come up");
    io::stdout().flush().ok();
    for _ in 0..30 {
        if let Some(ip) = get_vpn_interface_ip() {
            println!("\n{BGREEN}VPN is up. Your HTB IP address: {ip}{RESET}");
            return;
        }
        print!(".");
        io::stdout().flush().ok();
        thread::sleep(Duration::from_secs(1));
    }
    println!();
    eprintln!("\x1B[33mThe VPN interface did not come up within 30 seconds. Check `sudo journalctl -u openvpn` or run `sudo openvpn --config ~/lab-vpn.ovpn` in the foreground to see the error.\x1B[0m");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Example body documented for GET /api/v5/connections (community spec),
    // including a ProLab entry with null location_type_friendly/assigned_server.
    fn sample() -> Value {
        json!({"data": [
            {"type": "labs", "location_type_friendly": "US - VIP+",
             "assigned_server": {"id": 289, "friendly_name": "US VIP+ 1", "current_clients": 146, "location": "US"}},
            {"type": "starting_point", "location_type_friendly": "US - Starting Point",
             "assigned_server": {"id": 441, "friendly_name": "US StartingPoint 2", "current_clients": 107, "location": "US"}},
            {"type": "fortresses", "location_type_friendly": "US - Fortress",
             "assigned_server": {"id": 19, "friendly_name": "US Fortress 1", "current_clients": 8, "location": "US"}},
            {"type": "prolab", "pro_lab": {"id": 25, "name": "Mythical", "identifier": "MYTHICAL"},
             "location_type_friendly": null, "assigned_server": null},
            {"type": "competitive", "available": true, "location_type_friendly": "US - Release Arena",
             "assigned_server": {"id": 268, "friendly_name": "US Release Arena 1", "current_clients": 47, "location": "US"}}
        ]})
    }

    #[test]
    fn null_fields_do_not_abort_parsing() {
        let conns = parse_connections(&sample()).expect("should parse");
        assert_eq!(conns.len(), 5);
        let prolab = conns.iter().find(|c| c.access_type == "prolab").unwrap();
        assert!(prolab.location_type.is_none());
        assert!(prolab.server.is_none());
    }

    #[test]
    fn each_product_resolves_to_its_server() {
        let conns = parse_connections(&sample()).unwrap();
        for (kind, id) in [(VpnKind::Labs, 289), (VpnKind::StartingPoint, 441), (VpnKind::Fortresses, 19)] {
            let c = conns.iter().find(|c| c.access_type == kind.api_type()).unwrap();
            assert_eq!(c.server.as_ref().unwrap().id, id);
        }
    }

    #[test]
    fn removed_endpoint_message_is_reported() {
        let body = json!({"message": "This endpoint has been removed. ... consider using /api/v5/connections instead."});
        let err = parse_connections(&body).unwrap_err();
        assert!(err.contains("endpoint has been removed"));
    }

    #[test]
    fn unassigned_product_is_skipped_not_fatal() {
        let body = json!({"data": [
            {"type": "labs", "location_type_friendly": null, "assigned_server": null}
        ]});
        let conns = parse_connections(&body).unwrap();
        assert_eq!(conns.len(), 1);
        assert!(conns[0].server.is_none());
    }
}
