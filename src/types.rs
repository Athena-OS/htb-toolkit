use crate::api::fetch_api_async;
use crate::colors::*;
use crate::utils::get_vpn_interface_ip;
use core::time::Duration;
use std::fs;
use std::process;
use std::thread::sleep;

/// Extract the target IP from a `/api/v5/virtual_machine/active` body.
/// Returns Ok(None) while the VM is still spawning (`info.ip` is null) and
/// Err(()) when there is no active machine at all (`info` is null/absent).
fn active_vm_ip(json_data: &serde_json::Value) -> Result<Option<String>, ()> {
    let info = json_data.get("info").filter(|i| !i.is_null()).ok_or(())?;
    let ip = info
        .get("ip")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "null")
        .map(|s| s.to_string());
    Ok(ip)
}

/// Poll `/api/v5/virtual_machine/active` until the active VM has an IP.
///
/// Works the same for free, VIP/VIP+ and Starting Point machines: the v5
/// endpoint reports `ip: null` plus `isSpawning: true` until the box is ready.
/// (The old code special-cased Starting Point via `/api/v4/sp/profile/{id}`
/// and VIP via the account's VPN name; neither is needed any more.)
pub async fn get_ip(appkey: &str) -> String {
    let call_api: &str = "https://labs.hackthebox.com/api/v5/virtual_machine/active";
    let max_attempts = 20; // 20 x 15s = 5 min cap
    let wait = Duration::from_secs(15);

    for attempt in 1..=max_attempts {
        match fetch_api_async(call_api, appkey).await {
            Ok(json_data) => match active_vm_ip(&json_data) {
                Ok(Some(ip)) => return ip,
                Ok(None) => {
                    if attempt < max_attempts {
                        println!(
                            "Machine is spawning, no IP yet... waiting {}s ({attempt}/{max_attempts})",
                            wait.as_secs()
                        );
                        sleep(wait);
                    }
                }
                Err(()) => {
                    eprintln!("\x1B[31mNo active machine detected.\x1B[0m");
                    return String::new();
                }
            },
            Err(err) => {
                if err.is_timeout() {
                    eprintln!("Encountered timeout");
                } else {
                    eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
                }
                process::exit(1); // Exit with a non-zero status code
            }
        }
    }

    eprintln!("\x1B[31mCould not retrieve the machine IP address after {max_attempts} attempts.\x1B[0m");
    String::new()
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ActiveMachine {
    pub id: u64,
    pub name: String,
    pub ip: String,
    pub mtype: String,
}

impl ActiveMachine {
    pub async fn get_active(appkey: &str) -> Self {

        let call_api: &str = "https://labs.hackthebox.com/api/v5/virtual_machine/active";

        let result = fetch_api_async(call_api, appkey);
        
        //println!("Result: {:?}", result); // DEBUG: Print the result before the match

        match result.await {
            Ok(json_data) => {
                //println!("Fetched JSON Data: {:?}", json_data); // Print the fetched JSON data
                if let Some(info) = json_data.get("info") {
                    if info.is_null() {
                        eprintln!("\x1B[31mNo active machine detected.\x1B[0m");
                        //process::exit(1); // Exit with a non-zero status code. It interrupts the entire program
                        return ActiveMachine {
                            id: 0,
                            name: String::new(),
                            ip: String::new(),
                            mtype: String::new(),
                        };
                    }
                }
                let entry = &json_data["info"];
                let id = entry["id"].as_u64().unwrap_or(0);
                let name = entry["name"].as_str().unwrap_or("Name not available").to_string();
                // v5 reports the IP inline (null while spawning); don't block here.
                let ip = entry["ip"].as_str().unwrap_or_default().to_string();
                let mtype = entry["type"].as_str().unwrap_or("null").to_string();

                ActiveMachine {
                    id,
                    name,
                    ip,
                    mtype,
                }         
            }
            Err(err) => {
                if err.is_timeout() {
                    eprintln!("Encountered timeout");
                } else {
                    eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
                }
                process::exit(1); // Exit with a non-zero status code
            }
        }
    }
}

pub trait CommonTrait {
    fn get_name(&self) -> &str;
    fn get_avatar(&self) -> &str;
}

#[derive(Clone)]
pub struct SPMachine {
    pub id: u64,
    pub name: String,
    pub difficulty_str: String,
    pub tier: u64,
    pub avatar: String,
}

impl CommonTrait for SPMachine {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_avatar(&self) -> &str {
        &self.avatar
    }
}

#[derive(Clone)]
pub struct Machine {
    pub id: u64,
    pub name: String,
    pub points: u64,
    pub difficulty_str: String,
    pub user_pwn: bool,
    pub root_pwn: bool,
    pub free: bool,
    pub avatar: String,
}

impl CommonTrait for Machine {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_avatar(&self) -> &str {
        &self.avatar
    }
}

pub struct PlayingMachine {
    pub machine: Machine,
    pub sp_flag: bool,
    pub os: String,
    pub ip: String,
    pub review: bool,
}

impl PlayingMachine {

    /*const MACHINE: Machine = Machine {
            id: 0,
            name: String::new(),
            points: 0,
            difficulty_str: String::new(),
            user_pwn: String::new(),
            root_pwn: String::new(),
            free: false,
            avatar: String::new(),
        };

    pub fn new() -> Self {
        PlayingMachine {
            machine: Self::MACHINE.clone(),
            sp_flag: false,
            os: String::new(),
            ip: String::new(),
            review: false,
        }
    }*/

    pub fn get_os_icon(name: &str, os: &String, pos: &str) -> String {
        let icon_str: String;
        
        if pos == "right" {
            if os == "Linux" {
                icon_str = format!("{name} ");
            } else if os == "Windows" {
                icon_str = format!("{name} 󰖳");
            } else {
                icon_str = name.to_string();
            }
        } else if pos == "left" {
            if os == "Linux" {
                icon_str = format!(" {name}");
            } else if os == "Windows" {
                icon_str = format!("󰖳 {name}");
            } else {
                icon_str = name.to_string();
            }
        }
        else {
            icon_str = name.to_string();
        }
        icon_str
    }

    pub async fn get_machine(machine_name: &str, appkey: &str) -> Self {

        let base_api: &str = "https://labs.hackthebox.com/api/v4/machine/profile/";
        let call_api = format!("{base_api}{machine_name}");    
        let result = fetch_api_async(&call_api, appkey);
    
        let tiers = 3;

        match result.await {
            Ok(json_data) => {
                if let Some(message) = json_data.get("message").and_then(|m| m.as_str()) {
                    if message.contains("Machine not found") {
                        println!("\x1B[31m{message}.\x1B[0m");
                        println!("\x1B[31mSearching for a Starting Point Machine...\x1B[0m");

                        for index in 1..=tiers {
                            let tier_url = format!("https://labs.hackthebox.com/api/v5/machines?spTier={index}");
                            let sub_result = fetch_api_async(&tier_url, appkey);
                        
                            match sub_result.await {
                                Ok(sub_json_data) => {
                                    let entries = sub_json_data["data"].as_array().cloned().unwrap_or_default();
                                    for entry in entries.iter() {
                                        let id = entry["id"].as_u64().unwrap_or(0);
                                        let name = entry["name"].as_str().unwrap_or("Name not available");
                                        // v5 /machines uses `points`; keep `static_points` as a fallback for older payloads
                                        let points = entry["points"].as_u64()
                                            .or_else(|| entry["static_points"].as_u64())
                                            .unwrap_or(0);
                                        let os = entry["os"].as_str().unwrap_or("OS not available").to_string();
                                        let difficulty_str = entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string();
                                        let avatar_path = entry["avatar"].as_str().unwrap_or("Avatar not available").to_string();
                                        let user_pwn = entry["authUserInUserOwns"].as_bool()
                                            .or_else(|| entry["userOwn"].as_bool())
                                            .unwrap_or(false);
                                        let root_pwn = entry["authUserInRootOwns"].as_bool()
                                            .or_else(|| entry["rootOwn"].as_bool())
                                            .unwrap_or(false);
                                        let sp_flag = true;
                                        let ip = String::new(); // Will be retrieved later
                                        let machine_name_os_icon = Self::get_os_icon(name, &os, "right");

                                        if name.eq_ignore_ascii_case(machine_name) {
                                            println!("Found machine: {name}");
                                            return PlayingMachine {
                                                machine: Machine {
                                                    id,
                                                    name: machine_name_os_icon,
                                                    points,
                                                    difficulty_str,
                                                    user_pwn,
                                                    root_pwn,
                                                    free: true,
                                                    avatar: avatar_path,
                                                },
                                                sp_flag,
                                                os,
                                                ip,
                                                review: false,
                                            };
                                        }
                                    }
                                }
                                Err(err) => {
                                    if err.is_timeout() {
                                        eprintln!("Encountered timeout");
                                    } else {
                                        eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
                                    }
                                }
                            }
                        }
                        // Searched all tiers without a match: don't fall through to
                        // the regular-machine parsing below (it would panic on a
                        // "Machine not found" body).
                        eprintln!("\x1B[31mNo machine named '{machine_name}' found among active or Starting Point machines.\x1B[0m");
                        process::exit(1);
                    } else {
                        eprintln!("\x1B[31mAPI error for machine '{machine_name}': {message}\x1B[0m");
                        process::exit(1); // Exit with a non-zero status code
                    }
                }

                // Not SP Machines
                let entry = &json_data["info"];
                let id = match entry["id"].as_u64() {
                    Some(id) => id,
                    None => {
                        eprintln!("\x1B[31mUnexpected response for machine '{machine_name}': {json_data}\x1B[0m");
                        process::exit(1);
                    }
                };
                let name = entry["name"]
                            .as_str()
                            .unwrap_or("Name not available")
                            .to_string();
                let os = entry["os"]
                            .as_str()
                            .unwrap_or("null")
                            .to_string();
                let machine_name_os_icon = Self::get_os_icon(&name, &os, "right");

                PlayingMachine {
                    machine: Machine {
                        id,
                        name: machine_name_os_icon,
                        points: entry["points"].as_u64().unwrap_or(0),
                        difficulty_str: entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string(),
                        user_pwn: entry["authUserInUserOwns"].as_bool().unwrap_or(false),
                        root_pwn: entry["authUserInRootOwns"].as_bool().unwrap_or(false),
                        free: entry["free"].as_bool().unwrap_or(false),
                        avatar: entry["avatar"]
                            .as_str()
                            .unwrap_or("Avatar not available")
                            .to_string(),
                    },
                    sp_flag: false,
                    os,
                    ip: entry["ip"].as_str().unwrap_or_default().to_string(),
                    review: entry["authUserHasReviewed"].as_bool().unwrap_or(false),
                }         
            }
            Err(err) => {
                if err.is_timeout() {
                    eprintln!("Encountered timeout");
                } else {
                    eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
                }
                process::exit(1); // Exit with a non-zero status code
            }
        }
    }

    pub fn print_machine(m: PlayingMachine) {
        println!("Name: {}", m.machine.name);
        println!("IP Address: {}", m.ip);
        println!("Points: {}", m.machine.points);
        println!("Difficulty: {}", m.machine.difficulty_str);
        println!("User Pwned: {}", m.machine.user_pwn);
        println!("Root Pwned: {}", m.machine.root_pwn);
    }
}

#[derive(Clone)]
pub struct User {
    pub name: String,
}

impl User {
    pub async fn get_user(appkey: &str) -> Self {
        // Only the username is needed; the old extra call to
        // /user/profile/basic/{id} fed a VIP-VPN heuristic that is gone now.
        match fetch_api_async("https://labs.hackthebox.com/api/v4/user/info", appkey).await {
            Ok(json_user) => User {
                name: json_user["info"]["name"].as_str().unwrap_or("unknown").to_string(),
            },
            Err(err) => {
                if err.is_timeout() {
                    eprintln!("Encountered timeout");
                } else {
                    eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
                }
                process::exit(1); // Exit with a non-zero status code
            }
        }
    }
}

pub struct PlayingUser {
    pub user: User,
    pub ip: String,
}

impl PlayingUser {

    // get_playinguser looks for the VPN tunnel interface (tun0/tun1/wg*) to get the attacker IP address
    pub async fn get_playinguser(appkey: &str) -> Self {
        let mut userip: String = String::new();
        let account = User::get_user(appkey).await;
        // Retrieve User IP address
        match get_vpn_interface_ip() {
            Some(ip) => {
                userip = ip;
            }
            None => println!("{RED}Failed to retrieve the VPN interface IP address. Be sure your HTB VPN is active.{RESET}"),
        }

        PlayingUser {
            user : account,
            ip: userip,
        }
    }
}

pub struct HTBConfig {
    pub promptchange: bool,
}

impl HTBConfig {

    pub fn get_current_config(htb_config: &str) -> Self {
        HTBConfig {
            promptchange: Self::get_prompt_change(htb_config),
        }
    }

    fn get_prompt_change(htb_config: &str) -> bool {
        let prompt_change = fs::read_to_string(htb_config).expect("Failed to read htconfig.");

        let change_prompt = prompt_change.lines()
            .find(|line| line.starts_with("prompt_change="))
            .map(|line| line.split('=').nth(1).unwrap_or_default())
            .unwrap_or_default();

        // Convert the change_prompt string to a bool
        
        match change_prompt {
            "true" => true,
            "false" => false,
            _ => {
                // Handle other cases if needed, e.g., return a default value
                false
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn active_vm_ip_states() {
        // no active machine
        assert_eq!(active_vm_ip(&json!({"info": null})), Err(()));
        assert_eq!(active_vm_ip(&json!({})), Err(()));
        // spawning: ip null
        assert_eq!(
            active_vm_ip(&json!({"info": {"id": 1, "name": "Lame", "type": "Starting Point", "isSpawning": true, "ip": null}})),
            Ok(None)
        );
        // ready
        assert_eq!(
            active_vm_ip(&json!({"info": {"id": 1, "name": "Lame", "isSpawning": false, "ip": "10.129.1.2"}})),
            Ok(Some("10.129.1.2".to_string()))
        );
    }
}
