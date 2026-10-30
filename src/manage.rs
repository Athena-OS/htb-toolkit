use crate::appkey::get_appkey;
use crate::appkey::set_appkey;
use crate::appkey::reset_appkey;
use crate::appkey::delete_appkey;
use crate::colors::*;
use crate::list::*;
use crate::types::*;
use crate::utils::*;
use reqwest::Client;
use serde_json::Value;
use std::env;
use std::fs;
use std::io::{self,Write};
use std::path::PathBuf;
use regex::Regex;
use tokio::spawn;

pub async fn get_active_machine_info() {
    let appkey = get_appkey();

    let active_machine = ActiveMachine::get_active(&appkey).await;

    // Checking if the active_machine.name exists (so there is an active machine running) to avoid a generic error on api.rs file invoked by fetch_api in get_machine(). The check is done here and not inside ActiveMachine code because only here I need to exit if I don't have an Active Machine running
    if !active_machine.name.is_empty() {
        let mut machine_info = PlayingMachine::get_machine(&active_machine.name, &appkey).await;
        if machine_info.sp_flag {
            machine_info.ip = get_ip(&appkey).await;
        }
        
        PlayingMachine::print_machine(machine_info);
    }
    else {
        println!("Exiting...");
    }
}

pub async fn reset_machine() {
    let appkey = get_appkey();
    let active_machine = ActiveMachine::get_active(&appkey).await;

    // Example of moving the blocking operation to a separate task
    let appkey_clone = appkey.clone(); // Clone the necessary data
    let active_machine_clone = active_machine.clone(); // Clone other data if needed

    let blocking_task = spawn(async move {
        let client = Client::new();
        let reset_data = serde_json::json!({"machine_id": active_machine_clone.id});
        
        // Perform the HTTP request asynchronously
        match client
            .post("https://labs.hackthebox.com/api/v4/vm/reset")
            .header("Authorization", format!("Bearer {appkey_clone}"))
            .json(&reset_data)
            .send()
            .await
        {
            Ok(reset_response) => {
                if reset_response.status().is_success() {                    
                    //let reset_message = reset_response.text().await.expect("Failed to get response text."); //Print the response as text for debug
                    let reset_message = reset_response.json::<serde_json::Value>().await.unwrap_or(serde_json::Value::Null);
                    let reset_message = reset_message.get("message").and_then(|m| m.as_str()).unwrap_or("Machine reset requested.");
                    println!("{BGREEN}{reset_message}{RESET}");
                } else {
                    eprintln!("Failed to reset the machine. HTTP status code: {}", reset_response.status());
                }
            }
            Err(err) => {
                eprintln!("Error on POST request: {err:?}");
            }
        }
    });

    // Await the result of the blocking task
    blocking_task.await.expect("Blocking task failed");

    let active_machine = ActiveMachine::get_active(&appkey).await; //Declared 2nd time because reset machine
    let mut machine_info = PlayingMachine::get_machine(active_machine.name.as_str(), &appkey).await;
    if machine_info.ip.is_empty() { //Starting Point case because SP IP address is assigned only after spawn of the machine
        machine_info.ip = active_machine.ip;
    }
    let mut user_info = PlayingUser::get_playinguser(&appkey).await;

    // SP Machines change IP address when reset, so need to ask to write /etc/hosts
    if machine_info.sp_flag {
        let _ = add_hosts(&machine_info);
    }

    change_shell(&mut machine_info, &mut user_info);
}

pub async fn stop_machine() {
    let htb_path = format!("{}/.htb.conf", env::var("HOME").unwrap());
    let htbconfig = HTBConfig::get_current_config(&htb_path);
    let appkey = get_appkey();
    let active_machine = ActiveMachine::get_active(&appkey).await;

    // Example of moving the blocking operation to a separate task
    let appkey_clone = appkey.clone(); // Clone the necessary data
    let active_machine_clone = active_machine.clone(); // Clone other data if needed

    if !active_machine_clone.name.is_empty() { //If there is an active machine, stop it
        // vm/terminate works for free, VIP and Starting Point machines alike.
        let post_req = String::from("https://labs.hackthebox.com/api/v4/vm/terminate");

        let blocking_task = spawn(async move {
            let client = Client::new();
            let stop_data = serde_json::json!({"machine_id": active_machine_clone.id});
            let stop_response = client
                .post(post_req)
                .header("Authorization", format!("Bearer {appkey_clone}"))
                .json(&stop_data)
                .send()
                .await
                .expect("Error on POST request.");

            let status = stop_response.status();
            let stop_message = stop_response.json::<serde_json::Value>().await.unwrap_or(serde_json::Value::Null);
            let stop_message = stop_message.get("message").and_then(|m| m.as_str()).unwrap_or("Machine termination requested.");
            if status.is_success() {
                println!("{BGREEN}{stop_message}{RESET}");
            } else {
                eprintln!("{RED}Failed to stop the machine (HTTP {status}): {stop_message}{RESET}");
            }
        });

        // Await the result of the blocking task
        blocking_task.await.expect("Blocking task failed");

        if htbconfig.promptchange { //If the prompt is set to change during the playing, when you stop the machine, it should restore the original shell
            restore_shell();
        }
    }
}

pub fn prompt_setting(option: &str) {
    let home = env::var("HOME").unwrap_or_default();
    let htb_config = format!("{home}/.htb.conf");

    let content = fs::read_to_string(&htb_config)
        .expect("Failed to read HTB config file");

    let re = Regex::new(r"prompt_change=\w+")
        .expect("Failed to create regular expression");

    let new_content = re.replace(&content, format!("prompt_change={option}"));

    fs::write(&htb_config, new_content.to_string())
        .expect("Failed to write updated content to HTB config file");

    println!("Prompt setting updated to: {option}");
}

fn find_menu_children_mut<'a>(value: &'a mut Value, name: &str) -> Option<&'a mut Vec<Value>> {
    let obj = value.as_object_mut()?;

    // Check if current node has the target name
    if let Some(Value::String(existing_name)) = obj.get("name") {
        if existing_name == name {
            return obj.get_mut("children")?.as_array_mut();
        }
    }

    // Recursively search in children
    if let Some(Value::Array(children)) = obj.get_mut("children") {
        for child in children {
            if let Some(found) = find_menu_children_mut(child, name) {
                return Some(found);
            }
        }
    }

    None
}

pub async fn update_machines() -> Result<(), Box<dyn std::error::Error>> {

    println!("Retrieving updated data from Hack The Box... Gimme some time hackerzzz...");
    let home = env::var("HOME").unwrap_or_default();
    //let input_config = format!("{}/.input_config.txt", home);
    //let output_config = format!("{}/.output_config.txt", home);
    let menu_path = PathBuf::from(format!("{home}/.config/kando/menus.json"));

    // Free Machines
    let free_machine_list = list_machines("free").await;
    let fly_entries = htb_machines_to_flypie(free_machine_list).await;

    let menu_data = fs::read_to_string(&menu_path)?;
    let mut menu_json: Value = serde_json::from_str(&menu_data)?;

    // Navigate to root menu node
    let root_menu = menu_json
        .get_mut("menus")
        .and_then(|menus| menus.get_mut(0))
        .and_then(|menu| menu.get_mut("root"));

    // Find "Free Machines" children and replace them
    if let Some(root) = root_menu {
        if let Some(free_machines_children) = find_menu_children_mut(root, "Free Machines") {
            //println!("✅ Found 'Free Machines' — updating children...");
            *free_machines_children = fly_entries;
        } else {
            return Err("❌ Couldn't find 'Free Machines' in the menu structure.".into());
        }
    } else {
        return Err("❌ Couldn't find root menu.".into());
    }

    // Save the updated JSON
    let new_menu_str = serde_json::to_string_pretty(&menu_json)?;
    fs::write(&menu_path, new_menu_str)?;


    // Starting Point Machines
    let sp_machine_list = list_sp_machines().await;

    let tiers = 3;
    for index in 0..tiers {
        let tier_name = format!("Tier {index}"); // "Tier 0", "Tier 1", "Tier 2"

        // Filter machines by current tier
        let tiered_list: Vec<SPMachine> = sp_machine_list
            .iter()
            .filter(|machine| machine.tier == index)
            .cloned()
            .collect();

        let fly_entries = htb_machines_to_flypie(tiered_list).await;

        let menu_data = fs::read_to_string(&menu_path)?;
        let mut menu_json: Value = serde_json::from_str(&menu_data)?;

        // Navigate to root menu node
        let root_menu = menu_json
            .get_mut("menus")
            .and_then(|menus| menus.get_mut(0))
            .and_then(|menu| menu.get_mut("root"));

        // Go to "Starting Point Machines" first
        if let Some(root) = root_menu {
            if let Some(sp_children) = find_menu_children_mut(root, "Starting Point Machines") {
                // Then, within it, go to "Tier N"
                if let Some(tier_children) = sp_children
                    .iter_mut()
                    .find(|child| child.get("name") == Some(&Value::String(tier_name.clone())))
                    .and_then(|tier| tier.get_mut("children"))
                    .and_then(|children| children.as_array_mut())
                {
                    *tier_children = fly_entries;
                } else {
                    return Err(format!("❌ Couldn't find tier menu: {tier_name}").into());
                }
            } else {
                return Err("❌ Couldn't find 'Starting Point Machines' in the menu structure.".into());
            }
        } else {
            return Err("❌ Couldn't find root menu.".into());
        }

        // Save changes
        let new_menu_str = serde_json::to_string_pretty(&menu_json)?;
        fs::write(&menu_path, new_menu_str)?;
    }

    print!("\n{BGREEN}Machines updated. Press Enter to continue...{RESET}");
    let mut input = String::new();
    io::stdout().flush().expect("Flush failed!");
    io::stdin().read_line(&mut input).expect("Failed to read line");

    Ok(())
}

pub fn manage_app_key(option: &str) {
    if option == "set" {
        set_appkey();
    }
    else if option == "reset" {
        reset_appkey();
    }
    else if option == "delete" {
        delete_appkey();
    }
}