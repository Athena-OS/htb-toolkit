use crate::api::fetch_api_async;
use crate::appkey::get_appkey;
use crate::colors::*;
use crate::utils::*;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;
use reqwest::Client;
use tokio::spawn;

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
    location_type: String,
    can_access: bool,
    server: Option<VpnServer>,
}

async fn get_vpn_connections() -> Result<Vec<VpnAccess>, String> {
    let appkey = get_appkey();
    let result = fetch_api_async("https://labs.hackthebox.com/api/v5/connections", &appkey).await;

    match result {
        Ok(json_value) => {
            let mut connections = Vec::new();

            // `data` is an ARRAY of connection entries, not an object
            let data = json_value
                .get("data")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid `data` array")?;

            for entry in data {
                // Each entry is identified by its `type` field
                let access_type = match entry.get("type").and_then(|v| v.as_str()) {
                    Some(t) => t.to_string(),
                    None => continue, // skip malformed entries
                };

                // `available` only appears on some entries (e.g. competitive).
                // Absence means the entry is simply listed/accessible.
                let can_access = entry
                    .get("available")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let server = entry.get("assigned_server").and_then(|s| {
                    Some(VpnServer {
                        id: s.get("id")?.as_i64()?,
                        friendly_name: s.get("friendly_name")?.as_str()?.to_string(),
                        current_clients: s.get("current_clients")?.as_i64()?,
                        location: s.get("location")?.as_str()?.to_string(),
                    })
                });

                let location_type = entry
                    .get("location_type_friendly")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing location_type_friendly")?
                    .to_string();

                connections.push(VpnAccess {
                    access_type,
                    location_type,
                    can_access,
                    server,
                });
            }

            Ok(connections)
        }
        Err(err) => Err(format!("API error: {:?}", err)),
    }
}

async fn vpn_type() -> Option<Vec<String>> {
    let appkey = get_appkey();
    let result = fetch_api_async("https://labs.hackthebox.com/api/v4/connection/status", &appkey);
    let mut vpntype: Vec<String> = Vec::new();

    match result.await {
        Ok(json_value) => {
            if let Some(json_vpn) = json_value.as_array() {
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
    
    if get_interface_ip("tun0").is_some() {
        Some(vpntype)
    } else {
        None
    }
}

pub async fn check_vpn(machine_spflag: bool) {
    if let Some(vpntypes) = vpn_type().await {
        let vpntypes_str = vpntypes.join(", ");
        let mut yn = String::new();
        
        if vpntypes.len() > 1 {
            println!(
                "\nThe following VPN types are already running: {vpntypes_str}. You have multiple VPNs running. The oldest one will go down automatically in some minutes."
            );
        } else {
            println!(
                "\nThe following VPN type is already running: {vpntypes_str}."
            );
        }

        print!("Do you want to terminate the listed VPN and connect to a different one (y/n)? ");
        io::stdout().flush().expect("Flush failed!");
        io::stdin().read_line(&mut yn).expect("Failed to read input");
        
        match yn.trim() {
            "y" | "Y" => {
                run_vpn(machine_spflag).await;
            }
            _ => {}
        }
    } else {
        run_vpn(machine_spflag).await;
    }
}

pub async fn run_vpn(is_starting_point: bool) {
    let appkey = get_appkey();
    
    // Get available connections
    let connections = match get_vpn_connections().await {
        Ok(conns) => conns,
        Err(e) => {
            eprintln!("\x1B[31mFailed to get VPN connections: {}\x1B[0m", e);
            std::process::exit(1);
        }
    };

    // Filter based on type
    let filtered: Vec<VpnAccess> = connections.into_iter().filter(|c| {
        if is_starting_point {
            c.access_type == "starting_point"
        } else {
            c.access_type == "labs"
        }
    }).collect();

    if filtered.is_empty() {
        eprintln!("\x1B[31mNo VPN access available for this type.\x1B[0m");
        std::process::exit(1);
    }

    // Display available connections
    println!("\n{BCYAN}Available VPN Connections:{RESET}");
    for (idx, conn) in filtered.iter().enumerate() {
        if let Some(server) = &conn.server {
            println!("{}. {} - {} ({} users)", 
                idx + 1, 
                conn.location_type, 
                server.friendly_name,
                server.current_clients
            );
        }
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

    let server = match &selected.server {
        Some(s) => s.clone(),
        None => {
            eprintln!("\x1B[31mNo server assigned.\x1B[0m");
            std::process::exit(1);
        }
    };

    // Ask for TCP/UDP
    let mut vpn_tcp_flag = 0; // 0 = UDP, 1 = TCP
    loop {
        let mut input = String::new();
        print!("\n{BGREEN}Would you like to connect to Hack The Box VPN by UDP or TCP? [UDP] {RESET}");
        io::stdout().flush().expect("Flush failed!");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        input = input.trim().to_string();

        if input.is_empty() {
            input = "udp".to_string();
        }

        match input.to_lowercase().as_str() {
            "udp" => break,
            "tcp" => {
                vpn_tcp_flag = 1;
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

        let ovpn_url = format!(
            "https://labs.hackthebox.com/api/v4/access/ovpnfile/{}/{}",
            server.id, vpn_tcp_flag
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

        let ovpn_content = response.text().await.unwrap();
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

    if started {
        thread::sleep(Duration::from_secs(5));
    } else {
        std::process::exit(1);
        //println!("\n{BGREEN}You are running OpenVPN in background.{RESET}");
        //println!("To terminate it, close this window or run: sudo killall openvpn");
    }
}