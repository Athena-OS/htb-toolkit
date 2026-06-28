//mod only on main.rs or lib.rs , not on siblings. On siblings, use crate::
mod api;
mod appkey;
mod colors;
mod list;
mod manage;
mod play;
mod types;
mod utils;
mod vpn;
use std::env;
use crate::list::*;
use crate::manage::*;
use crate::play::*;
use crate::utils::*;
use crate::vpn::*;
use std::fs;
use std::path::Path;
use std::process::Command;

#[tokio::main]
async fn main() {

    check_root();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // Handle the case where no command-line arguments are provided
        match print_banner() {
            Ok(_) => {}
            Err(error) => {
                eprintln!("Error: {error}");
            }
        }
        get_help();
        return;
    }

    // Initialization
    // Create avatar folder for free machine icons
    let home = env::var("HOME").unwrap_or_default();
    let folder_path = format!("{home}/.local/share/icons/htb-toolkit/avatar");

    if fs::metadata(&folder_path).is_err() { // Create the folder only if it does not exist
        if let Err(err) = fs::create_dir_all(&folder_path) {
            eprintln!("Error creating folder: {err}");
        }
    }
    
    // Create HTB config file if not existing
    let htb_config = format!("{home}/.htb.conf");

    let file = Path::new(&htb_config);
    if !file.exists() {
        let lines = ["# HTB configuration file.\n\n", "# Enable/Disable shell prompt change\n", "prompt_change=true\n"];
        fs::write(&htb_config, lines.join(""))
            .expect("Failed to create HTB config file");
    }

    // Initialize Xorg in WSL for secret-tool popup window
    if is_wsl() && is_display_zero() {
        Command::new("sh")
            .arg("-c")
            .arg("source /etc/X11/xinit/xinitrc.d/50-systemd-user.sh 2> /dev/null")
            .status()
            .expect("Failed to execute shell command");
    }
    //////////////////////////////////

    match args[1].as_str() {
        "-h" => {
            match print_banner() {
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Error: {error}");
                }
            }
            get_help();
        }
        "-a" => {
            get_active_machine_info().await;
        }
        "-f" => {
            submit_flag().await;
        }
        "-k" => {
            if args.len() < 3 || (args[2] != "set" && args[2] != "reset" && args[2] != "delete") {
                println!("Usage: {} -k <set|reset|delete>", args[0]);
            } else {
                manage_app_key(&args[2]);
            }
        }
        "-l" => {
            if args.len() < 3 || (args[2] != "free" && args[2] != "retired" && args[2] != "starting") {
                println!("Usage: {} -l <free|retired|starting>", args[0]);
            } else if args[2] == "free" || args[2] == "retired" {
                list_machines(&args[2]).await; //Use await because inside it I use asycn fetch_api
            } else if args[2] == "starting" {
                list_sp_machines().await;
            } 
        }
        "-m" => {
            if args.len() < 3 {
                println!("Usage: {} -m <machine-name>", args[0]);
            } else {
                let _ = play_machine(&args[2]).await;
            }
        }
        "-p" => {
            if args.len() < 3 || (args[2] != "true" && args[2] != "false") {
                println!("Usage: {} -p <true|false>", args[0]);
            } else {
                prompt_setting(&args[2]);
            }
        }
        "-r" => {
            reset_machine().await;
        }
        "-s" => {
            stop_machine().await;
        }
        "-u" => {
            let _ = update_machines().await;
        }
        "-v" => {
            if args.len() < 3 {
                println!("Usage: {} -v <lab|starting|fortress>", args[0]);
                println!("\nVPN Types:");
                println!("  lab            - Connect to Machines VPN");
                println!("  starting       - Connect to Starting Point VPN");
                println!("  fortress       - Connect to Fortress VPN");
            } else {
                let is_starting_point = match args[2].as_str() {
                    "starting" | "sp" => true,
                    "labs" | "machine" | "machines" => false,
                    "fortress" | "fort" => {
                        eprintln!("\x1B[33mNote: Fortress VPN will use lab connection.\x1B[0m");
                        false
                    }
                    _ => {
                        eprintln!("\x1B[31mInvalid VPN type: {}\x1B[0m", args[2]);
                        println!("Valid types: lab, starting, fortress");
                        std::process::exit(1);
                    }
                };
                run_vpn(is_starting_point).await;
            }
        }
        _ => {
            match print_banner() {
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Error: {error}");
                }
            }
            println!("Invalid command: {}", args[1]);
            get_help();
        }
    }
}
