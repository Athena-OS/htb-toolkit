use crate::api::fetch_api_async;
use crate::colors::*;
use crate::utils::get_interface_ip;
use core::time::Duration;
use std::fs;
use std::process;
use std::thread::sleep;

pub async fn get_ip (appkey: &str) -> String {
    let call_api: &str = "https://labs.hackthebox.com/api/v5/virtual_machine/active";

    let result = fetch_api_async(call_api, appkey);
    let mut machine_ip = String::new();

    let account = User::get_user(appkey).await;
        
    //println!("Result: {:?}", result); // DEBUG: Print the result before the match

    match result.await {
        Ok(json_data) => {
            //println!("Fetched JSON Data: {:?}", json_data); // Print the fetched JSON data
            if let Some(info) = json_data.get("info") {
                if info.is_null() {
                    eprintln!("\x1B[31mNo active machine detected.\x1B[0m");
                    //process::exit(1); // Exit with a non-zero status code. It interrupts the entire program
                    return machine_ip;
                }
                if let Some(type_value) = info.get("type").and_then(|t| t.as_str()) {

                    if type_value.contains("Starting Point") || account.vpnname.contains("VIP") { //If the machine is Starting Point type or if you are using a VIP or VIP+ VPN, the machine needs some min to generate the IP address (even if the machine is free)
                        let mut get_req = String::new();
                        if type_value.contains("Starting Point") {
                            get_req = format!(                        
                                "https://labs.hackthebox.com/api/v4/sp/profile/{}",                        
                                &json_data["info"]["id"]
                            );
                        }
                        else if account.vpnname.contains("VIP") {
                            get_req = format!(                        
                                "https://labs.hackthebox.com/api/v4/machine/profile/{}",                        
                                &json_data["info"]["name"].as_str().unwrap()
                            );
                        }

                        loop {
                            let sub_result = fetch_api_async(&get_req, appkey);

                            match sub_result.await {
                                Ok(sub_json) => {
                                    machine_ip = sub_json["info"]["ip"].as_str().unwrap_or_default().to_string();

                                    if !machine_ip.is_empty() && machine_ip != "null" {
                                        return machine_ip;
                                    }
                                    println!("Retrieving machine IP address... Wait 30 seconds...");
                                    sleep(Duration::from_secs(30));
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
                    else {
                        let machine_name = info.get("name").and_then(|t| t.as_str()).expect("Machine name not found").to_string();
                        let machine_info = PlayingMachine::get_machine(&machine_name, appkey).await;
                        machine_ip = machine_info.ip;
                        return machine_ip;
                    }
                }
            }
            machine_ip
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

#[derive(Clone)]
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
                let id = entry["id"].as_u64().unwrap();
                let name = entry["name"].as_str().unwrap_or("Name not available").to_string();
                let ip = get_ip(appkey).await;
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
                                    for entry in sub_json_data["data"].as_array().unwrap().iter() {
                                        let id = entry["id"].as_u64().unwrap();
                                        let name = entry["name"].as_str().unwrap_or("Name not available");
                                        let points = entry["static_points"].as_u64().unwrap();
                                        let os = entry["os"].as_str().unwrap_or("OS not available").to_string();
                                        let difficulty_str = entry["difficultyText"].as_str().unwrap_or("Difficulty not available").to_string();
                                        let avatar_path = entry["avatar"].as_str().unwrap_or("Avatar not available").to_string();
                                        let user_pwn = entry["userOwn"]
                                            .as_bool().unwrap_or(false);
                                        let root_pwn = entry["rootOwn"]
                                            .as_bool().unwrap_or(false);
                                        let sp_flag = true;
                                        let ip = String::new(); // Will be retrieved later
                                        let machine_name_os_icon = Self::get_os_icon(name, &os, "right");
                                        
                                        if name == machine_name {
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
                    } else {
                        eprintln!("\x1B[31mError fetching Starting Point data.\x1B[0m");
                        process::exit(1); // Exit with a non-zero status code
                    }
                }
                
                // Not SP Machines
                let entry = &json_data["info"];
                let id = entry["id"].as_u64().unwrap();
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
                    ip: entry["ip"].as_str().unwrap_or("null").to_string(),
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
    pub vpnname: String,
}

impl User {
    /*pub fn new() -> Self {
        User {
            id: 0,
            name: String::new(),
            vpnname: String::new(),
        }
    }*/

    pub async fn get_user(appkey: &str) -> Self {
        let id: u64;
        let username: String;
        let vpnname: String;

        // Retrieve User username
        let result = fetch_api_async("https://labs.hackthebox.com/api/v4/user/info", appkey);
    
        match result.await {
            Ok(json_user) => {
                id = json_user["info"]["id"].as_u64().unwrap();
                username = json_user["info"]["name"].as_str().unwrap().to_string();

                let user_id_url = format!("https://labs.hackthebox.com/api/v4/user/profile/basic/{id}");
                let details = fetch_api_async(&user_id_url, appkey);
    
                match details.await {
                    Ok(json_details) => {
                        vpnname = json_details["profile"]["server"].as_str().unwrap().to_string();
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
            Err(err) => {
                if err.is_timeout() {
                    eprintln!("Encountered timeout");
                } else {
                    eprintln!("\x1B[31mError. Maybe your API key is incorrect or expired. Renew your API key by running htb-toolkit -k reset.\x1B[0m");
                }
                process::exit(1); // Exit with a non-zero status code
            }
        }
    
        User {
            name: username,
            vpnname,
        }
    }
}

pub struct PlayingUser {
    pub user: User,
    pub ip: String,
}

impl PlayingUser {

    /*const USER: User = User {
            id: 0,
            name: String::new(),
            vpnname: String::new(),
        };
    
    pub fn new() -> Self {
        PlayingUser {
            user: Self::USER.clone(),
            ip: String::new(),
        }
    }*/

    // get_playinguser fetches for tun0 interface for attacker IP address
    pub async fn get_playinguser(appkey: &str) -> Self {
        let mut userip: String = String::new();
        let account = User::get_user(appkey).await;
        // Retrieve User IP address
        let interface_name = "tun0";
        let ip_address = get_interface_ip(interface_name);
    
        match ip_address {
            Some(ip) => {
                userip = ip;
            }
            None => println!("{RED}Failed to retrieve IP address of {interface_name}. Be sure your HTB VPN is active.{RESET}"),
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