# HTB Toolkit

![image](https://github.com/D3vil0p3r/htb-toolkit/assets/83867734/1455a5db-fa91-485b-91ba-bb27675357b9)

**HTB Toolkit** allows you to play Hack The Box machines directly on your system.

# Usage

To use HTB Toolkit, you need to retrieve an **App Token** from your Hack The Box [Profile Settings](https://app.hackthebox.com/profile/settings) and click on **Create App Token** button under **App Tokens** section.

Once generated and copied to clipboard, run in the terminal:
```bash
htb-toolkit -k set
```
After the **Password:** prompt, paste the App Token value and press **Enter**. It will be stored securely in your system keyring.

**Don't share your App Token with anyone!**

## Troubleshooting API Token Storage

If you experience issues with the keyring (e.g., `secret-tool` errors), HTB Toolkit supports a fallback configuration file:

1. Create the config directory:
   ```bash
   mkdir -p ~/.config/htb-toolkit
   ```

2. Store your App Token:
   ```bash
   echo "YOUR_APP_TOKEN_HERE" > ~/.config/htb-toolkit/token
   chmod 600 ~/.config/htb-toolkit/token
   ```

The toolkit will automatically use this fallback if the keyring is unavailable.

**Note**: The keyring method is preferred for security. Only use the config file as a fallback.

## VPN Connection

HTB Toolkit automatically connects to Hack The Box VPN servers. **HTB now automatically assigns VPN servers based on your account location and subscription level** - you no longer need to manually select specific servers like "EUFree1" or "USFree1".

To connect to a VPN:
```bash
# For Machines (Labs)
htb-toolkit -v lab

# For Starting Point
htb-toolkit -v starting

# For Fortresses
htb-toolkit -v fortress
```

The toolkit will:
1. Query HTB API to get your assigned server(s)
2. Display available options for your region
3. Auto-connect if only one server is assigned
4. Let you choose if multiple servers are available (VIP users)

**Example output:**
```
Available VPN Connections:
1. AU - Machines - AU Machines 1 (22 users)

Auto-selecting the only available server.
Connecting to AU Machines 1 [id=177]
VPN config file saved successfully.
OpenVPN started successfully
```

## Playing Machines

Once connected to VPN, you can spawn and play machines:
```bash
# Play a specific machine
htb-toolkit -m <machine-name>

# List available free machines
htb-toolkit -l free

# List retired machines
htb-toolkit -l retired

# List starting point machines
htb-toolkit -l starting
```

Showcase of HTB Toolkit:

[![HTB Toolkit Asciicast](https://github.com/D3vil0p3r/htb-toolkit/assets/83867734/cfc8aac4-f58e-4b44-8ac1-12e1842c801f)](https://asciinema.org/a/605148)

Interactive source: [Asciinema](https://asciinema.org/a/605148)

## Troubleshooting API Token Storage

If you experience issues with the keyring (e.g., `secret-tool` errors), HTB Toolkit supports a fallback configuration file:

1. Create the config directory:
````bash
   mkdir -p ~/.config/htb-toolkit
````

2. Store your App Token in the config file:
````bash
   echo "YOUR_APP_TOKEN_HERE" > ~/.config/htb-toolkit/token
   chmod 600 ~/.config/htb-toolkit/token
````

The toolkit will automatically use this fallback if the keyring is unavailable.

**Note**: The keyring method is preferred for security. Only use the config file as a fallback.

# Install

## Arch-based Linux distro

Add [Athena OS](https://athenaos.org/) repository to your system as described [here](https://athenaos.org/en/configuration/repositories/#installation).

Run:
```bash
sudo pacman -Syyu
sudo pacman -S htb-toolkit
```

# Build from source

## Non-Arch-based Linux distro

### Install Runtime Dependencies

**Arch-based distros**
```bash
sudo pacman -S coreutils gnome-keyring gzip libsecret noto-fonts-emoji openssl openvpn ttf-nerd-fonts-symbols
```

**Debian-based distros**
```bash
sudo apt install coreutils fonts-noto-color-emoji gnome-keyring gzip libsecret-tools libssl-dev openvpn

# Ensure gnome-keyring daemon is running (required for API token storage)
eval $(gnome-keyring-daemon --start --components=secrets 2>/dev/null)

# Add to your shell startup file for persistence (~/.bashrc, ~/.zshrc, etc.)
echo 'eval $(gnome-keyring-daemon --start --components=secrets 2>/dev/null)' >> ~/.bashrc

# Install Nerd Fonts for proper icon display
wget https://github.com/ryanoasis/nerd-fonts/releases/latest/download/NerdFontsSymbolsOnly.zip
unzip NerdFontsSymbolsOnly.zip -x LICENSE readme.md -d ~/.fonts
fc-cache -fv
```

**Why is `libsecret-tools` needed?**

HTB Toolkit uses `secret-tool` (part of `libsecret-tools`) to securely store your API token in the system keyring. This prevents your token from being stored in plain text. If `secret-tool` is not available, the toolkit will automatically fall back to storing the token in `~/.config/htb-toolkit/token` (see Troubleshooting section above).

### Install Build Dependencies

```bash
# Debian/Ubuntu
sudo apt install git cargo

# Arch Linux
sudo pacman -S git cargo
```

### Build

Clone the repository:
```bash
git clone https://github.com/D3vil0p3r/htb-toolkit
cd htb-toolkit
cargo build --release
```

This will create the binary file **htb-toolkit** in `htb-toolkit/target/release`. 

Install it system-wide:
```bash
sudo cp htb-toolkit/target/release/htb-toolkit /usr/bin/
```

Now you can run:
```bash
htb-toolkit -h
```

# FlyPie Integration in Athena OS

HTB Toolkit can be integrated into the FlyPie menu of Athena OS using the `htb-toolkit -u` command. This will implement **shell-rocket** as terminal wrapper inside the FlyPie menu HTB machine icons to run HTB machines.

# VPN Architecture Changes (2025)

**Important:** As of January 2025, Hack The Box changed their VPN infrastructure. The old system with manually selectable servers (EUFree1, USFree1, EUVIP1, etc.) has been replaced with automatic server assignment.

**What changed:**
- ❌ Old: Manual server selection from ~100+ servers
- ✅ New: Automatic assignment based on your location and subscription

**How it works now:**
1. HTB analyzes your account (location, subscription level)
2. HTB assigns the optimal server(s) for your region
3. The toolkit connects to your assigned server automatically
4. VIP users may see multiple options in their region to choose from
5. Free users typically get one server per region

**Migration from old syntax:**
```bash
# Old (deprecated)
htb-toolkit -v EUFree1

# New
htb-toolkit -v lab
```

This change ensures better load balancing and optimal connection speeds for all users.

# Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

# License

This project is licensed under the GPL-3.0 License. See the LICENSE file for details.
