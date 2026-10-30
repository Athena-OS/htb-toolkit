use crate::appkey::get_appkey;
use crate::colors::*;
use crate::manage::*;
use crate::types::*;
use crate::utils::*;
use crate::vpn::*;
use std::env;
use std::io::{self,Write};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json,Value};
use tokio::spawn;

#[derive(Serialize)]
struct ReviewData {
    id: u64,
    stars: u32,
    headline: String,
    review: String,
}

#[derive(Serialize)]
struct FlagData {
    flag: String,
    id: u64,
    /// 10..=100 in steps of 10; the v5 endpoint expects an integer
    difficulty: u32,
}

pub async fn play_machine(machine_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let appkey = get_appkey();
    let appkey_clone = appkey.clone(); // Clone the necessary data to avoid borrowed value error
    let htb_path = format!("{}/.htb.conf", env::var("HOME").unwrap());
    let htbconfig = HTBConfig::get_current_config(&htb_path);
    
    let mut machine_info = PlayingMachine::get_machine(machine_name, &appkey).await;

    println!("Stopping any active machine...");
    println!("{BYELLOW}Note: if you interrupted the htb-toolkit before the spawn of a previous machine, give me two minutes to end the old spawn process and stop the related machine...{RESET}");
    stop_machine().await;

    check_vpn(machine_info.sp_flag).await;    

    let blocking_task = spawn(async move {
        //For SP Machines and VIP VPN (not Free VPN)
        let client = Client::new();
        let response_result = client
            .post("https://labs.hackthebox.com/api/v4/vm/spawn")
            .json(&json!({
                "machine_id": machine_info.machine.id
            }))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {appkey}"))
            .send()
            .await;
        
        match response_result {
            Ok(response) => {
                let response_status = response.status();
                if response_status.is_success() {
                    let message = response.text().await.expect("Failed to get response text");
                    let json: Value = serde_json::from_str(&message).expect("Failed to parse JSON");
                    let message = json["message"]
                        .as_str()
                        .expect("Missing or invalid 'message' field");
                    println!("{BGREEN}{message}{RESET}");
                } else if response_status.as_u16() == 400 {
                    // Show why vm/spawn refused (e.g. "already have an active machine",
                    // "VPN not connected") instead of silently trying the legacy call.
                    let spawn_text = response.text().await.unwrap_or_default();
                    let spawn_msg = serde_json::from_str::<Value>(&spawn_text)
                        .ok()
                        .and_then(|j| j["message"].as_str().map(|s| s.to_string()))
                        .unwrap_or(spawn_text);
                    eprintln!("{BYELLOW}vm/spawn refused: {spawn_msg}{RESET}");
                    eprintln!("Trying the legacy free-tier play endpoint...");

                    // For Free VPN (legacy)
                    let post_req = format!("https://labs.hackthebox.com/api/v4/machine/play/{}", machine_info.machine.id);
                    let response_play = client
                        .post(post_req)
                        .header("Authorization", format!("Bearer {appkey}"))
                        .send()
                        .await;
                    
                    match response_play {
                        Ok(subresponse) => {
                            let subresponse_status = subresponse.status();
                            if subresponse_status.is_success() {
                                let subresponse_text = subresponse.text().await.expect("Failed to get response text");
                                println!("Response: {subresponse_text}");
                            } else {
                                let subresponse_text = subresponse.text().await.expect("Failed to get response text");
                                println!("Response: {subresponse_text}");
                                eprintln!("Request failed with status: {subresponse_status}");
                                std::process::exit(1);
                            }
                        }
                        Err(err) => {
                            eprintln!("Error on POST request: {err:?}");
                        }
                    }
                } else {
                    let response_text = response.text().await.expect("Failed to get response text");
                    println!("Response: {response_text}");
                    eprintln!("Request failed with status: {response_status}");
                    std::process::exit(1);
                }
            }
            Err(err) => {
                eprintln!("Error on POST request: {err:?}");
            }
        }
    });

    // Await the result of the blocking task
    blocking_task.await.expect("Blocking task failed");
    
    machine_info.ip = get_ip(&appkey_clone).await; // For Starting Point machines and VIP and VIP+ VPNs, if I call the play API two times on the same machine, the old IP address associated to the machine can still live for some seconds providing a wrong IP related to the new same machine. For this reason, it is better to compute always the IP address (no problems for free VPNs because they associate always the same IP address to the same machine)

    let mut user_info = PlayingUser::get_playinguser(&appkey_clone).await; // Before this it is needed to run HTB VPN to take the Attacker IP address

    let _ = print_banner();

    if machine_info.machine.user_pwn {
        println!("{BGREEN}Hey! You have already found the User Flag! Nice one!{RESET}");
    }

    if machine_info.machine.root_pwn {
        println!("{BGREEN}Hey! You have already found the Root Flag! Keep it up!{RESET}");
    }

    if htbconfig.promptchange { //If the prompt is set to change during the playing...
        change_shell(&mut machine_info, &mut user_info);
    }

    // Writing /etc/hosts
    let _ = add_hosts(&machine_info);

    display_target_info(&machine_info, &user_info);

    Ok(())
}

pub async fn submit_flag() {
    let appkey = get_appkey();
    let appkey_clone = appkey.clone(); // Clone the necessary data to avoid borrowed value error
    let mut flag = String::new();
    let mut machine_name = String::new();
    let mut machine_rating = String::new();

    println!("{BGREEN}Did you get a flag? Please, submit it and continue your hacking path. Good Luck!{RESET}");

    print!("{RED}Submit the flag:{RESET} ");
    io::stdout().flush().expect("Flush failed!");
    io::stdin()
        .read_line(&mut flag)
        .expect("Failed to read line");

    print!("{BGREEN}Specify the machine name:{RESET} ");
    io::stdout().flush().expect("Flush failed!");
    io::stdin()
        .read_line(&mut machine_name)
        .expect("Failed to read line");

    let machine_info = PlayingMachine::get_machine(machine_name.trim(), &appkey).await;

    println!("{BCYAN}How much is the difficulty of this machine?{RESET}");
    println!();
    println!("{BGREEN}[10 --> Piece of Cake!]{RESET}");
    println!("{BGREEN}[20 --> Very Easy!]{RESET}");
    println!("{BGREEN}[30 --> Easy!]{RESET}");
    println!("{BYELLOW}[40 --> Not Too Easy!]{RESET}");
    println!("{BYELLOW}[50 --> Medium!]{RESET}");
    println!("{BYELLOW}[60 --> A Bit Hard!]{RESET}");
    println!("{BYELLOW}[70 --> Hard!]{RESET}");
    println!("{RED}[80 --> Too Hard!]{RESET}");
    println!("{RED}[90 --> Extremely Hard!]{RESET}");
    println!("{RED}[100 --> Brainfuck!]{RESET}");
    println!();

    
    print!("{BYELLOW}Difficulty Rating:{RESET} ");
    io::stdout().flush().expect("Flush failed!");
    io::stdin()
        .read_line(&mut machine_rating)
        .expect("Failed to read line");

    let difficulty: u32 = match machine_rating.trim().parse::<u32>() {
        Ok(d) if (10..=100).contains(&d) && d % 10 == 0 => d,
        _ => {
            eprintln!("{RED}Difficulty must be one of 10, 20, ..., 100.{RESET}");
            std::process::exit(1);
        }
    };

    let blocking_task = spawn(async move {
        let flag_data = FlagData {
            flag: flag.trim().to_string(),
            id: machine_info.machine.id,
            difficulty,
        };

        // Create a reqwest client
        let client = Client::new();

        // Send the POST request to submit the flag (moved from v4 to v5)
        let response = client
            .post("https://labs.hackthebox.com/api/v5/machine/own")
            .json(&flag_data)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {appkey}"))
            .send();
  
        match response.await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    // Print the response text
                    let response_text = resp.text().await.expect("Failed to read response text");
                    println!("{response_text}");
                } else {
                    // Status 400: Incorrect flag
                    let response_text = resp.text().await.expect("Failed to read response text");
                    let parsed_response: Result<serde_json::Value, _> = serde_json::from_str(&response_text);

                    if let Ok(json) = parsed_response {
                        if let Some(message) = json.get("message").and_then(|m| m.as_str()) {
                            println!("{message}");
                        }
                    } else {
                        eprintln!("Request failed with status: {status}");
                        std::process::exit(1);
                    }
                }
            }
            Err(err) => {
                eprintln!("Error sending request: {err}");
            }
        }
    });

    // Await the result of the blocking task
    blocking_task.await.expect("Blocking task failed");

    if machine_info.machine.user_pwn && machine_info.machine.root_pwn && machine_info.review {
        println!("{}Wonderful! You PWNED {}! Would you like to submit feedback?{}", BYELLOW, machine_info.machine.name, RESET);

        loop {
            println!("Select an option:");
            println!("1. Yes");
            println!("2. No");
    
            let mut input = String::new();
            io::stdin().read_line(&mut input).expect("Failed to read input");
            let choice = input.trim();
    
            match choice {
                "1" => {
                    println!("{BGREEN}Please, write a headline for your feedback (max 50 chars):{RESET}");
                    let mut review_headline = String::new();
                    io::stdin().read_line(&mut review_headline).expect("Failed to read input");
    
                    println!("{BGREEN}Please, submit your feedback (max 2000 chars):{RESET}");
                    let mut review_machine = String::new();
                    io::stdin().read_line(&mut review_machine).expect("Failed to read input");
    
                    println!("{BGREEN}How many stars would you give to this machine?{RESET}");
                    print!("Stars (1 to 5): ");
    
                    let mut review_stars = String::new();
                    io::stdout().flush().expect("Flush failed!");
                    io::stdin().read_line(&mut review_stars).expect("Failed to read input");
                    let review_stars = review_stars.trim().parse::<u32>().unwrap_or(0);
    
                    let review_data = ReviewData {
                        id: machine_info.machine.id,
                        stars: review_stars,
                        headline: review_headline.trim().to_string(),
                        review: review_machine.trim().to_string(),
                    };
    
                    // Create a reqwest client
                    let client = Client::new();
    
                    // Send the POST request
                    let response = client
                        .post("https://labs.hackthebox.com/api/v4/machine/review")
                        .json(&review_data)
                        .header("Content-Type", "application/json")
                        .header("Authorization", format!("Bearer {appkey_clone}"))
                        .send()
                        .await
                        .expect("Failed to send request");
    
                    if response.status().is_success() {
                        let message: serde_json::Value = response.json().await.expect("Failed to parse JSON");
                        println!("{}", message["message"]);
                    } else {
                        println!("Request failed with status: {}", response.status());
                        std::process::exit(1);
                    }
    
                    break;
                }
                "2" => {
                    break;
                }
                _ => {
                    println!("Invalid choice. Please select a valid option.");
                }
            }
        }
    }
}