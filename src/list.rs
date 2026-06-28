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

                for entry in json_data["data"].as_array().unwrap().iter() {
                    let id = entry["id"].as_u64().unwrap();
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

pub async fn list_machines(machine_type: &str) -> Vec<Machine> {
    let mut machine_list: Vec<Machine> = Vec::new();

    println!("Listing machines... This operation could require some minutes...");
    println!("\n\x1B[93mConnecting to HTB server...\x1B[0m\n");

    let appkey = get_appkey(); // Retrieve the app key
    let mut last_page: u64 = 1;

    let result: Result<serde_json::Value, reqwest::Error> = match machine_type {
        "free" => fetch_api_async("https://labs.hackthebox.com/api/v4/machine/paginated", &appkey).await,
        "retired" => {
            let url = "https://labs.hackthebox.com/api/v4/machine/list/retired/paginated";
            let mut result_list: Vec<serde_json::Value> = Vec::new();
            let last_page_req = fetch_api_async(url, &appkey).await;
            
            if let Ok(json_meta) = last_page_req {
                //println!("Received JSON from retired call (page {}): {:?}", page_number, json_value);
                last_page = json_meta["meta"]["last_page"].as_u64().unwrap();
                //println!("{}", last_page);
            } else {
                eprintln!("Error fetching data for last page {last_page}");
            }

            for page_number in 1..=last_page {
                let url = format!(
                    "https://labs.hackthebox.com/api/v4/machine/list/retired/paginated?page={page_number}"
                );
                let page_result = fetch_api_async(&url, &appkey).await;

                // If the current call is successful, append the result to the list
                if let Ok(json_value) = page_result {
                    //println!("Received JSON from retired call (page {}): {:?}", page_number, json_value);
                    result_list.push(json_value);
                } else {
                    eprintln!("Error fetching data for page {page_number}");
                }
            }

            /*// Concatenate the JSON representations into a single string
            let concatenated_json = result_list
                .into_iter()
                .map(|value| value)
                .collect::<serde_json::Value>();
            
                    // Parse the JSON string into a serde_json::Value
            //println!("{}", concatenated_json[0]);
            //println!("{:?}", serde_json::from_str::<serde_json::Value>(&concatenated_json));
            Ok(concatenated_json)*/

            //Ok(serde_json::Value::String(concatenated_json))
            Ok(serde_json::Value::Array(result_list.into_iter().collect()))
        },
        _ => {
            eprintln!("\x1B[31mInvalid machine type: {machine_type}\x1B[0m");
            return machine_list;
        }
    };
    
    match result {
        Ok(json_data) => {
            //println!("Received JSON from retired call: {}", json_data[1]);
            println!("\x1B[92mDone.\x1B[0m\n");
            println!("\x1B[93mCalculating the number of machines...\x1B[0m\n");
            std::thread::sleep(std::time::Duration::from_secs(1));

            let mut array_free_machines: Vec<Vec<String>> = Vec::new();
            for _ in 0..last_page as usize {
                array_free_machines.push(Vec::new());
            }

            println!("\x1B[92mDone.\x1B[0m\n");
            if machine_type == "retired" {
                for page_number in 1..=last_page as usize {
                    for entry in json_data[page_number-1]["data"].as_array().unwrap().iter() {

                        let id = entry["id"].as_u64().unwrap_or(0);
                        let name = entry["name"].as_str().unwrap_or("Name not available").to_string();
                        let os = entry["os"].as_str().unwrap_or("OS not available").to_string();
                        let machine_name_os_icon = PlayingMachine::get_os_icon(&name, &os, "left");
                        let points = entry["points"].as_u64().unwrap_or(0);
                        let difficulty_str = entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string();
                        let user_pwn = entry["authUserInUserOwns"].as_bool().unwrap_or(false);
                        let root_pwn = entry["authUserInRootOwns"].as_bool().unwrap_or(false);
                        let free = entry["free"].as_bool().unwrap_or(false);
                        let avatar_path = entry["avatar"].as_str().unwrap_or("Avatar not available").to_string();

                        if free {
                            array_free_machines[page_number-1].push(name);
                        }

                        let machine = Machine {
                            id,
                            name: machine_name_os_icon,
                            points,
                            difficulty_str,
                            user_pwn,
                            root_pwn,
                            free,
                            avatar: avatar_path,
                        };

                        machine_list.push(machine);
                    }
                }
                println!();
                println!("\x1B[92mToday, the free retired machines are:\x1B[0m\n");

                for page_number in 1..=last_page as usize {
                    for freename in &array_free_machines[page_number - 1] {
                        println!("{freename}");
                    }
                }
                println!("\n");
            } else {
                // For "free" machines, json_data is the direct API response with "data" field
                let data_array = match json_data.get("data").and_then(|d| d.as_array()) {
                    Some(arr) => arr,
                    None => {
                        eprintln!("\x1B[31mERROR: Failed to parse 'data' field for free machines\x1B[0m");
                        eprintln!("Response structure: {}", serde_json::to_string_pretty(&json_data).unwrap_or_default());
                        return machine_list;
                    }
                };
                
                for entry in data_array.iter() {

                    let id = entry["id"].as_u64().unwrap_or(0);
                    let name = entry["name"].as_str().unwrap_or("Name not available").to_string();
                    let os = entry["os"].as_str().unwrap_or("OS not available").to_string();
                    let machine_name_os_icon = PlayingMachine::get_os_icon(&name, &os, "left");
                    let points = entry["points"].as_u64().unwrap_or(0);
                    let difficulty_str = entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string();
                    let user_pwn = entry["authUserInUserOwns"].as_bool().unwrap_or(false);
                    let root_pwn = entry["authUserInRootOwns"].as_bool().unwrap_or(false);
                    let free = entry["free"].as_bool().unwrap_or(false);
                    let avatar_path = entry["avatar"].as_str().unwrap_or("Avatar not available").to_string();

                    let machine = Machine {
                        id,
                        name: machine_name_os_icon,
                        points,
                        difficulty_str,
                        user_pwn,
                        root_pwn,
                        free,
                        avatar: avatar_path,
                    };

                    machine_list.push(machine);
                }
            }

            display_table(&machine_list);
        }
        Err(err) => {
            if err.is_timeout() {
                eprintln!("Encountered timeout");
            } else {
                eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
            }
        }
    }
    machine_list
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