use serde_json::json;
use serde_json::Value;
use std::process::{Command, exit, Stdio};
use std::io::{self, Write};
use std::env;
use crate::colors::*;
use crate::types::*;
use pnet::datalink;
use regex::Regex;
use reqwest::Client;
use std::fs;
use std::net::IpAddr;
use std::path::Path;
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use users::{get_current_uid};

pub fn check_root() {
    let current_uid = get_current_uid();
    if current_uid == 0 {
        eprintln!("Please do not run this application by root or sudo.");
        exit(1);
    }
}

// HTB's VPN usually comes up as tun0, but it can be tun1, tun2, or a
// WireGuard-style interface. Try the common names first, then fall back to any
// interface whose name looks like a VPN tunnel. Never panics.
fn get_vpn_interface_ip() -> Option<String> {
    for name in ["tun0", "tun1", "tun2"] {
        if let Some(ip) = get_interface_ip(name) {
            return Some(ip);
        }
    }
    for iface in datalink::interfaces() {
        if iface.name.starts_with("tun") || iface.name.starts_with("wg") {
            for addr in &iface.ips {
                if let IpAddr::V4(v4) = addr.ip() {
                    return Some(v4.to_string());
                }
            }
        }
    }
    None
}

pub fn change_shell(machine_info: &mut PlayingMachine, user_info: &mut PlayingUser) {
    let result = std::env::var("SHELL").unwrap_or_default();
    let mut file_bak = String::new();
    let mut file = String::new();
    let mut prompt = String::new();
    let mut prompt_field = "";

    if result.contains("bash") {
        file_bak = format!("{}/.bashrc.htb.bak", std::env::var("HOME").unwrap_or_default());
        file = format!("{}/.bashrc", std::env::var("HOME").unwrap_or_default());
        prompt = format!(
            "PS1=\"\\e[32m\\]┌──[Target:{}🚀🌐IP:{}🔥\\e[34m\\]Attacker:{}📡IP:{}\\e[32m\\]🏅Prize:{} points]\\n└──╼[👾]\\\\[\\e[36m\\]\\$(pwd) $ \\[\\e[0m\\]\"",
            machine_info.machine.name,
            machine_info.ip,
            user_info.user.name,
            get_vpn_interface_ip().unwrap_or_else(|| "N/A".to_string()),
            machine_info.machine.points
        );
        prompt_field = "PS1=.*";
    } else if result.contains("fish") {
        file_bak = format!("{}/.config/fish/functions/fish_prompt.fish.htb.bak", std::env::var("HOME").unwrap_or_default());
        file = format!("{}/.config/fish/functions/fish_prompt.fish", std::env::var("HOME").unwrap_or_default());
        prompt = format!(
            r#"function fish_prompt
    set_color 00ff00
    echo -n "┌──[Target:{}🚀🌐IP:{}"
    set_color ff00d7
    echo -n "🔥Attacker:{}📡IP:{}"
    set_color 00ff00
    echo "🏅Prize:{} points]"
    set_color 00ff00
    echo -n "└──╼[👾]"
    set_color 00ffff
    echo (pwd) '$' (set_color normal)
end"#,
            machine_info.machine.name,
            machine_info.ip,
            user_info.user.name,
            get_vpn_interface_ip().unwrap_or_else(|| "N/A".to_string()),
            machine_info.machine.points
        );
    } else if result.contains("zsh") {
        file_bak = format!("{}/.zshrc.htb.bak", std::env::var("HOME").unwrap_or_default());
        file = format!("{}/.zshrc", std::env::var("HOME").unwrap_or_default());
        prompt = format!(
            "PROMPT=\"%F{{46}}┌──[Target:{}🚀🌐IP:{}🔥%F{{201}}Attacker:{}📡IP:{}%F{{46}}🏅Prize:{} points]\"$'\\n'\"└──╼[👾]%F{{44}}%~ $%f \"" ,
            machine_info.machine.name,
            machine_info.ip,
            user_info.user.name,
            get_vpn_interface_ip().unwrap_or_else(|| "N/A".to_string()),
            machine_info.machine.points
        );
        prompt_field = "PROMPT=.*";
    }

    if !std::path::Path::new(&file_bak).exists() {
        std::fs::copy(&file, &file_bak).unwrap_or_default();
    }
    
    if result.contains("bash") || result.contains("zsh") {
        let file_content = std::fs::read_to_string(&file).unwrap_or_default();
        let regex = Regex::new(prompt_field).unwrap();
        // NoExpand: the prompt string contains '$' (e.g. \$(pwd), '$ '), which the
        // regex engine would otherwise treat as capture-group references and mangle.
        let new_file_content = if regex.is_match(&file_content) {
            regex
                .replace_all(&file_content, regex::NoExpand(prompt.as_str()))
                .into_owned()
        } else {
            // No existing PS1/PROMPT line to replace — append ours so it still applies.
            format!("{file_content}\n{prompt}\n")
        };
        std::fs::write(&file, new_file_content).unwrap_or_default();
    } else if result.contains("fish") {
        std::fs::write(&file, &prompt).unwrap_or_default();
    }

    if !file.is_empty() {
        println!(
            "{BYELLOW}Prompt written to {file}. To apply it to your CURRENT shell run:{RESET}"
        );
        println!("{BGREEN}    exec $SHELL{RESET}   {BYELLOW}(or open a new terminal){RESET}");
        println!("{BYELLOW}Note: if you use starship, oh-my-zsh, or powerlevel10k, those frameworks regenerate the prompt on every command and will override this.{RESET}");
    }
}

pub fn restore_shell() {
    let result = env::var("SHELL").unwrap_or_default();
    let mut file_bak = String::new();
    let mut file = String::new();

    if result.contains("bash") {
        file_bak = format!("{}/.bashrc.htb.bak", env::var("HOME").unwrap());
        file = format!("{}/.bashrc", env::var("HOME").unwrap());
    } else if result.contains("fish") {
        file_bak = format!("{}/.config/fish/functions/fish_prompt.fish.htb.bak", env::var("HOME").unwrap());
        file = format!("{}/.config/fish/functions/fish_prompt.fish", env::var("HOME").unwrap());
    } else if result.contains("zsh") {
        file_bak = format!("{}/.zshrc.htb.bak", env::var("HOME").unwrap());
        file = format!("{}/.zshrc", env::var("HOME").unwrap());
    }
    if fs::metadata(&file).is_ok() && std::path::Path::new(&file_bak).exists() {
        //Restore the prompt file from the backup
        fs::copy(&file_bak, &file).expect("Failed to copy file");
    }
}

pub fn display_target_info(machine_info: &PlayingMachine, user_info: &PlayingUser) {
    println!();
    println!("{BYELLOW}Our secret agent gathered some information about the target:{RESET}");
    println!("{BGREEN}┌────────────────────────────────────────────────────┐{RESET}");
    println!("{}| Target Name       : {}{}{}", BGREEN, BCYAN, machine_info.machine.name, RESET);
    println!("{}| Target OS         : {}{}{}", BGREEN, BCYAN, machine_info.os, RESET);
    println!("{}| Target IP         : {}{}{}", BGREEN, BCYAN, machine_info.ip, RESET);
    println!("{}| Points            : {}{}{}", BGREEN, BCYAN, machine_info.machine.points, RESET);
    println!("{}| Difficulty        : {}{}{}", BGREEN, BCYAN, machine_info.machine.difficulty_str, RESET);
    println!("{}| User Flag         : {}{}{}", BGREEN, BCYAN, machine_info.machine.user_pwn, RESET);
    println!("{}| Root Flag         : {}{}{}", BGREEN, BCYAN, machine_info.machine.root_pwn, RESET);
    println!("{BGREEN}|────────────────────────────────────────────────────|{RESET}");
    println!("{}| Attacker          : {}{}{}", BGREEN, RED, user_info.user.name, RESET);
    println!("{}| Attacker IP       : {}{}{}", BGREEN, RED, user_info.ip, RESET);
    println!("{BGREEN}└────────────────────────────────────────────────────┘{RESET}");
    println!();
    println!("{BYELLOW}The agent left this information in the console.{RESET}\n");
}

pub fn get_interface_ip(interface_name: &str) -> Option<String> {
    // Get a list of network interfaces
    let interfaces = datalink::interfaces();

    // Find the desired interface by name
    if let Some(interface) = interfaces.into_iter().find(|iface| iface.name == interface_name) {
        // Iterate through the IP addresses of the interface
        for addr in &interface.ips {
            if let IpAddr::V4(ipv4) = addr.ip() { 
                return Some(ipv4.to_string())
            }
        }
    } else {
        println!("Interface not found: {interface_name}");
    }
    
    None // Return None if interface not found or IP not found
}

pub fn print_banner() -> Result<(), Box<dyn std::error::Error>> {
    let encoded = "H4sIAAAAAAAAA+1byw6CMBC8+xW9aAxpWvVm/BQ9eefu5/sgkQIFCrR02N09mYmH6c50uxRWqbixC/vb43V+3k+366U8Vj/LeYjtQnFY6lgM8ZFl2QrSRJB1ED1PRrRlpEPMJ8hubTtJeYNGP14ajBNo7BJ6O0R5ssVZG0/QXW5bbFdoNK5rIspng3/QLf3GYwU0kgl3w2h8fcEoIU0rEDkEx1Wuo0Ajv0RHWjJGy02ID9BIi52oI0O2JNt/cLW15dRUdBFdNVaNRguNY14k9wmUb2toOYCdmyjW26KocrDqZUx24xN6CpmHMG93nDORn/HZl358pNW4MfWoofySoIsUnodSvsoDVKme2wLRRDQRTUQT0UQ0EU1Ek7iabPIyypdFYRTfG3s0eukQLPEadlJ0L8+sbhruF2SfwQtEf6ElKR3ifhHf+RiW0fW8QhC+IrHJ7mO5+7wO3Gw3Ng8xubecoTvYZQ8ea/XP/JA9cAWZ3KL0eWR4TFIclLGMDUTACC7aeqLVwN4RuAnJWeDtwfnLmsMbuwsrDnU9AAA=";

    // Decode using base64
    let mut child = Command::new("base64")
        .arg("-d")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(encoded.as_bytes())?;
    }
    let output = child.wait_with_output()?;

    // Decompress using gunzip
    let mut gunzip_child = Command::new("gunzip")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = gunzip_child.stdin {
        stdin.write_all(&output.stdout)?;
    }
    let gunzip_output = gunzip_child.wait_with_output()?;

    if gunzip_output.status.success() {
        let decompressed = String::from_utf8_lossy(&gunzip_output.stdout).into_owned().replace("\\x1b", "\x1b"); // .replace is needed to apply the colors on the banner string
        
        let mut stdout = io::stdout();
        writeln!(stdout, "{decompressed}")?;
    } else {
        eprintln!("'gunzip' command failed");
    }

    Ok(())
}

pub fn get_help() {
    // Display Help
    println!("Play Hack The Box machines directly on your system.");
    println!();
    std::thread::sleep(std::time::Duration::from_secs(2));
    println!("{} [-h] [-a] [-f] [-k] <set|reset|delete> [-m] <machine-name> [-l] <free|retired|starting> [-p] <true|false> [-r] [-s] [-u] [-v] <lab|starting|fortress>", env::args().next().unwrap());
    println!();
    println!("Options:");
    println!("-a                            Print information about the current active machine.");
    println!("-f                            Submit a flag.");
    println!("-h                            Print this help.");
    println!("-k <set|reset|delete>         Set, reset or delete the Hack The Box App Key.");
    println!("-m <machine-name>             Specify the machine name to play.");
    println!("-l <free|retired|starting>    List free, retired or starting point machines.");
    println!("-p <true|false>               Set if the shell prompt should be changed.");
    println!("-r                            Reset the playing machine.");
    println!("-s                            Stop the playing machine.");
    println!("-u                            Update free machines in the Red Team menu.");
    println!("-v <lab|starting|fortress>    Connect to a Hack The Box VPN.");
    println!();
    println!("VPN Connection:");
    println!("HTB now automatically assigns VPN servers based on your location.");
    println!("Available VPN types:");
    println!("  lab            - Machines VPN (automatically assigned)");
    println!("  starting       - Starting Point VPN (automatically assigned)");
    println!("  fortress       - Fortress VPN (automatically assigned)");
    println!();
    println!("Usage Examples:");
    println!("{}", env::args().next().unwrap());
    println!("{} -k set", env::args().next().unwrap());
    println!("{} -l free", env::args().next().unwrap());
    println!("{} -m Eighteen", env::args().next().unwrap());
    println!("{} -u", env::args().next().unwrap());
    println!("{} -v labs", env::args().next().unwrap());
    println!("{} -v starting", env::args().next().unwrap());
}

pub fn is_inside_container() -> bool {
    if let Ok(cgroup) = fs::read_to_string("/proc/1/cgroup") {
        cgroup.contains("/docker/") || cgroup.contains("/podman/")
    } else {
        false
    }
}

pub fn is_wsl() -> bool {
    if let Ok(uname) = fs::read_to_string("/proc/sys/kernel/osrelease") {
        uname.contains("Microsoft") || uname.contains("WSL")
    } else {
        false
    }
}

pub fn is_display_empty() -> bool {
    if let Ok(display_value) = env::var("DISPLAY") {
        display_value.is_empty()
    } else {
        false
    }
}

pub fn is_display_zero() -> bool {
    if let Ok(display_value) = env::var("DISPLAY") {
        display_value == ":0"
    } else {
        false
    }
}

pub async fn htb_machines_to_flypie<T: CommonTrait>(
    machine_list: Vec<T>,
) -> Vec<Value> {
    let terminal = "shell-rocket -c";
    let (sender, mut receiver) = mpsc::channel(machine_list.len());
    let home = env::var("HOME").unwrap();
    let avatar_dir = format!("{home}/.config/kando/icon-themes/avatar");
    let _= fs::create_dir_all(&avatar_dir);

    for machine in machine_list.iter() {
        let machine_name = machine.get_name().split_once(' ').unwrap().1;
        let machine_avatar = machine.get_avatar().to_string();
        let avatar_url = format!("https://htb-mp-prod-public-storage.s3.eu-central-1.amazonaws.com{machine_avatar}");
        let avatar_filename = format!(
            "{avatar_dir}/{machine_name}.png"
        );

        let response = Client::new().get(&avatar_url).send().await;
        match response {
            Ok(response) => {
                if response.status().is_success() {
                    let image_data = response.bytes().await;
                    match image_data {
                        Ok(image_data) => {
                            let avatar_file = AsyncFile::create(&avatar_filename).await;
                            match avatar_file {
                                Ok(avatar_file) => {
                                    let mut writer = BufWriter::new(avatar_file);
                                    if writer.write_all(&image_data).await.is_ok() {
                                        let _ = sender.send(avatar_filename).await;
                                    }
                                }
                                _ => eprintln!("Failed to create file: {avatar_filename:?}"),
                            }
                        }
                        Err(err) => eprintln!("Failed to read image data: {err:?}"),
                    }
                } else {
                    eprintln!("Bad status code for: {avatar_url}");
                }
            }
            Err(err) => eprintln!("HTTP error for {avatar_url}: {err:?}"),
        }
    }

    let mut avatar_filenames = Vec::new();
    for _ in 0..machine_list.len() {
        let received_avatar = receiver.recv().await.expect("Receive error");
        avatar_filenames.push(received_avatar);
    }

    // Return Vec<Value> instead of formatted string
    machine_list
        .iter()
        .zip(avatar_filenames.iter())
        .map(|(machine, avatar_filename)| {
            let machine_name = machine.get_name().split_once(' ').unwrap().1;
            let icon_filename = Path::new(avatar_filename)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let machine_command = format!("{terminal} 'htb-toolkit -m {machine_name}'");
            json!({
                "name": machine_name,
                "icon": icon_filename, // kando needs only the filename, not the entire path
                "iconTheme": "avatar",
                "type": "command",
                "data": {
                    "command": machine_command
                },
                "angle": -1
            })
        })
        .collect()
}

pub fn add_hosts(machine_info: &PlayingMachine) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let mut yn = String::new();
        print!("\n{BGREEN}Would you like to assign a domain name to the target machine IP address and store it in /etc/hosts (y/n)? {RESET}");
        io::stdout().flush().expect("Flush failed!");
        io::stdin().read_line(&mut yn).expect("Failed to read input");

        match yn.trim() {
            "y" | "Y" => {
                // Safety net: never write a not-ready IP (empty or the literal
                // string "null") into /etc/hosts.
                if machine_info.ip.is_empty() || machine_info.ip == "null" {
                    println!("{RED}No valid IP address available yet — skipping /etc/hosts entry.{RESET}");
                    return Ok(());
                }

                let hosts_path = std::path::Path::new("/etc/hosts");
                let domain_name = format!("{}.htb", machine_info.machine.name.split_whitespace().next().unwrap_or_default().to_string().to_lowercase()); // Using this set of func to remove the os icon after the machine name
                print!("{BGREEN}Type the domain name to assign {RED}[{domain_name}]{BGREEN}: {RESET}");
                io::stdout().flush().expect("Flush failed!");

                let mut ans = String::new();
                io::stdin().read_line(&mut ans).expect("Failed to read input");
                ans = ans.trim().to_string();

                if ans.is_empty() {
                    ans = domain_name;
                }

                if is_inside_container() {
                    let mut hosts_content = format!("{}  {}\n", machine_info.ip, ans);
                    if let Ok(existing_content) = std::fs::read_to_string(hosts_path) {
                        if !existing_content.contains(&hosts_content) {
                            hosts_content = existing_content + &hosts_content;
                        }
                    }
                    std::fs::write("/tmp/hosts.new", hosts_content).expect("Failed to write to hosts.new");
                    std::process::Command::new("sudo")
                        .args(["cp", "-f", "/tmp/hosts.new", "/etc/hosts"])
                        .status()
                        .expect("Failed to copy hosts file");
                    std::fs::remove_file("/tmp/hosts.new").expect("Failed to remove hosts.new");
                } else {
                    // Read the current contents of the hosts file
                    let current_content = fs::read_to_string(hosts_path)?;
                    let new_entry = format!("{} {}", machine_info.ip, ans);
                    
                    // Check if the new entry already exists in the hosts file. If so, remove it because it could be placed at bottom than a more recent (and wrong) one
                    if current_content.contains(&new_entry) {
                        println!("Hosts file already contains the new entry. Removing old entry...");
                        let sed_remove_pattern = format!("/{new_entry}/d");

                        std::process::Command::new("sudo")
                            .args(["sed", "-i", &sed_remove_pattern, "/etc/hosts"])
                            .status()
                            .expect("Failed to edit hosts file");
                    }
                    let sed_pattern = format!("1i{new_entry}");
                    std::process::Command::new("sudo")
                        .args(["sed", "-i", &sed_pattern, "/etc/hosts"])
                        .status()
                        .expect("Failed to edit hosts file");
                }
                return Ok(());
            }
            "n" | "N" => return Ok(()),
            _ => println!("Invalid answer."),
        }
    }
}