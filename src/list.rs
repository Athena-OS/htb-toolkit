use crate::appkey::get_appkey;
use crate::api::fetch_api_async;
use crate::colors::*;
use crate::types::*;

pub async fn list_sp_machines() -> Vec<SPMachine> {
    let tiers = 3;

    println!("\n\x1B[93mConnecting to HTB server...\x1B[0m\n");
    let appkey = get_appkey();

    let mut all_sp_machine_list: Vec<SPMachine> = Vec::new();

    for index in 1..=tiers {
        let mut sp_machine_list: Vec<SPMachine> = Vec::new();
        let tier_lvl = index - 1;
        let tier_url = format!("https://labs.hackthebox.com/api/v5/machines?spTier={index}");
        let result = fetch_api_async(&tier_url, &appkey);

        match result.await {
            Ok(json_data) => {
                println!("\x1B[92mDone.\x1B[0m\n");

                let entries = json_data["data"].as_array().cloned().unwrap_or_default();
                for entry in entries.iter() {
                    let id = entry["id"].as_u64().unwrap_or(0);
                    let name = entry["name"].as_str().unwrap_or("Name not available").to_string();
                    let os = entry["os"].as_str().unwrap_or("OS not available").to_string();
                    let machine_name_os_icon = PlayingMachine::get_os_icon(&name, &os, "left");
                    let difficulty_str = entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string();
                    let avatar_path = entry["avatar"].as_str().unwrap_or("Avatar not available").to_string();

                    let sp_machine = SPMachine { id, name: machine_name_os_icon, difficulty_str, tier: tier_lvl, avatar: avatar_path };

                    sp_machine_list.push(sp_machine);
                }
                println!("{BYELLOW}Tier {tier_lvl} Starting Point machines:{RESET}\n");
                display_table_sp(&sp_machine_list);

                all_sp_machine_list.extend(sp_machine_list);
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
    
    all_sp_machine_list
}

/// Build a `Machine` from one entry of `/api/v5/machines`.
fn machine_from_entry(entry: &serde_json::Value) -> Machine {
    let name = entry["name"].as_str().unwrap_or("Name not available").to_string();
    let os = entry["os"].as_str().unwrap_or("OS not available").to_string();
    Machine {
        id: entry["id"].as_u64().unwrap_or(0),
        name: PlayingMachine::get_os_icon(&name, &os, "left"),
        points: entry["points"].as_u64().unwrap_or(0),
        difficulty_str: entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string(),
        user_pwn: entry["authUserInUserOwns"].as_bool().unwrap_or(false),
        root_pwn: entry["authUserInRootOwns"].as_bool().unwrap_or(false),
        free: entry["free"].as_bool().unwrap_or(false),
        avatar: entry["avatar"].as_str().unwrap_or("Avatar not available").to_string(),
    }
}

/// Fetch every page of `/api/v5/machines?state=<state>` (100 per page, the API max).
async fn fetch_all_machines(state: &str, appkey: &str) -> Result<Vec<Machine>, reqwest::Error> {
    let mut machines = Vec::new();
    let mut page: u64 = 1;

    loop {
        let url = format!(
            "https://labs.hackthebox.com/api/v5/machines?state={state}&per_page=100&page={page}"
        );
        let json_data = fetch_api_async(&url, appkey).await?;

        match json_data.get("data").and_then(|d| d.as_array()) {
            Some(arr) => machines.extend(arr.iter().map(machine_from_entry)),
            None => {
                eprintln!("\x1B[31mERROR: no 'data' array in /api/v5/machines response (page {page})\x1B[0m");
                eprintln!("Response: {}", serde_json::to_string_pretty(&json_data).unwrap_or_default());
                break;
            }
        }

        let last_page = json_data["meta"]["last_page"].as_u64().unwrap_or(1);
        if page >= last_page {
            break;
        }
        page += 1;
        println!("Fetched page {page}/{last_page}...");
    }

    Ok(machines)
}

pub async fn list_machines(machine_type: &str) -> Vec<Machine> {
    println!("Listing machines... This operation could require some minutes...");
    println!("\n\x1B[93mConnecting to HTB server...\x1B[0m\n");

    let appkey = get_appkey(); // Retrieve the app key

    // "free" historically meant the currently active (non-retired) machines,
    // which every tier can play; "retired" lists retired ones and flags the
    // subset that is free this week.
    let state = match machine_type {
        "free" => "active",
        "retired" => "retired",
        _ => {
            eprintln!("\x1B[31mInvalid machine type: {machine_type}\x1B[0m");
            return Vec::new();
        }
    };

    match fetch_all_machines(state, &appkey).await {
        Ok(machine_list) => {
            println!("\x1B[92mDone.\x1B[0m\n");

            if machine_type == "retired" {
                println!("\x1B[92mToday, the free retired machines are:\x1B[0m\n");
                for m in machine_list.iter().filter(|m| m.free) {
                    println!("{}", crate::utils::strip_os_icon(&m.name));
                }
                println!("\n");
            }

            display_table(&machine_list);
            machine_list
        }
        Err(err) => {
            if err.is_timeout() {
                eprintln!("Encountered timeout");
            } else {
                eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
            }
            Vec::new()
        }
    }
}

fn display_table_sp(machine_list: &[SPMachine]) {
    println!(
        "{}{:<8}{} {}{:<20}{} {}{:<15}{}",
        BBLUE, "ID", RESET, BGREEN, "Name", RESET, BCYAN, "Difficulty", RESET, 
    );
    println!("----------------------------------------");

    for machine in machine_list {
        println!(
            "{}{:<8}{} {}{:<20}{} {}{:<15}{}",
            BBLUE, machine.id, RESET,
            BGREEN, machine.name, RESET,
            BCYAN, machine.difficulty_str, RESET,
        );
    }
}

fn display_table(machine_list: &[Machine]) {
    println!(
        "{}{:<8}{} {}{:<20}{} {}{:<8}{} {}{:<15}{} {}{:<10}{} {}{:<10}{} {}{:<8}{}",
        BBLUE, "ID", RESET, BGREEN, "Name", RESET, BYELLOW, "Points", RESET, BCYAN, "Difficulty", RESET, BYELLOW, "User Pwned", RESET, RED, "Root Pwned", RESET, BGREEN, "Is it Free?", RESET
    );
    println!("-------------------------------------------------------------");

    for machine in machine_list {
        println!(
            "{}{:<8}{} {}{:<20}{} {}{:<8}{} {}{:<15}{} {}{:<10}{} {}{:<10}{} {}{:<8}{}",
            BBLUE, machine.id, RESET,
            BGREEN, machine.name, RESET,
            BYELLOW, machine.points, RESET,
            BCYAN, machine.difficulty_str, RESET,
            BYELLOW, machine.user_pwn, RESET,
            RED, machine.root_pwn, RESET,
            BGREEN, machine.free, RESET,
        );
    }
}