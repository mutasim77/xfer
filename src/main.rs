use clap::{App, Arg, SubCommand};
use colored::*;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    host: String,
    user: String,
    key_path: Option<String>,
    port: Option<u16>,
    default_remote_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    servers: HashMap<String, ServerConfig>,
    default_server: Option<String>,
}

impl Config {
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = home_dir()
            .unwrap_or_default()
            .join(".config")
            .join("xfer")
            .join("config.toml");

        if !config_path.exists() {
            return Ok(Config {
                servers: HashMap::new(),
                default_server: None,
            });
        }

        let content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = home_dir().unwrap_or_default().join(".config").join("xfer");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    fn get_server(&self, alias: &str) -> Option<&ServerConfig> {
        self.servers.get(alias)
    }
}

struct TransferEngine;

impl TransferEngine {
    fn parse_location(
        location_str: &str,
        config: &Config,
    ) -> Result<(String, String, String), String> {
        if !location_str.contains(':') {
            return Ok((
                "local".to_string(),
                "".to_string(),
                location_str.to_string(),
            ));
        }

        let parts: Vec<&str> = location_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err("Invalid location format. Use 'alias:/path/to/file'".to_string());
        }

        let alias = parts[0];
        let path = parts[1];

        let server = config.get_server(alias).ok_or_else(|| {
            format!(
                "Unknown server alias '{}'. Add it to your config first.",
                alias
            )
        })?;

        let remote_path = if path.starts_with('/') {
            path.to_string()
        } else if let Some(default_path) = &server.default_remote_path {
            format!("{}/{}", default_path, path)
        } else {
            format!("/home/{}/{}", server.user, path)
        };

        Ok((alias.to_string(), server.host.clone(), remote_path))
    }

    fn send_file(src: &str, dest: &str, config: &Config) -> Result<(), String> {
        let (src_alias, src_host, src_path) = Self::parse_location(src, config)?;
        let (dest_alias, dest_host, dest_path) = Self::parse_location(dest, config)?;

        if src_alias == "local" && dest_alias != "local" {
            let server = config.get_server(&dest_alias).unwrap();
            Self::transfer_to_remote(
                src_path,
                &dest_host,
                &server.user,
                &dest_path,
                server.key_path.as_deref(),
                server.port,
            )
        } else if src_alias != "local" && dest_alias == "local" {
            let server = config.get_server(&src_alias).unwrap();
            Self::transfer_from_remote(
                &src_host,
                &server.user,
                &src_path,
                dest_path,
                server.key_path.as_deref(),
                server.port,
            )
        } else if src_alias == "local" && dest_alias == "local" {
            Self::transfer_local_to_local(src_path, dest_path)
        } else {
            // TODO: Remote to remote transfer
            Err("Direct remote-to-remote transfers not supported yet".to_string())
        }
    }

    fn transfer_to_remote(
        local_path: String,
        host: &str,
        user: &str,
        remote_path: &str,
        key_path: Option<&str>,
        port: Option<u16>,
    ) -> Result<(), String> {
        let path = Path::new(&local_path);

        if path.is_dir() {
            Self::run_rsync(
                &format!("{}/", local_path),
                &format!("{}@{}:{}", user, host, remote_path),
                key_path,
                port,
            )
        } else {
            Self::run_scp(
                &local_path,
                &format!("{}@{}:{}", user, host, remote_path),
                key_path,
                port,
            )
        }
    }

    fn transfer_from_remote(
        host: &str,
        user: &str,
        remote_path: &str,
        local_path: String,
        key_path: Option<&str>,
        port: Option<u16>,
    ) -> Result<(), String> {
        Self::run_scp(
            &format!("{}@{}:{}", user, host, remote_path),
            &local_path,
            key_path,
            port,
        )
    }

    fn transfer_local_to_local(src: String, dest: String) -> Result<(), String> {
        let path = Path::new(&src);

        if path.is_dir() {
            let output = Command::new("rsync")
                .args(&["-av", "--progress", &src, &dest])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| format!("Failed to execute rsync: {}", e))?;

            if !output.status.success() {
                return Err(format!(
                    "rsync failed with exit code: {:?}",
                    output.status.code()
                ));
            }
        } else {
            let output = Command::new("cp")
                .args(&[&src, &dest])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(|e| format!("Failed to execute cp: {}", e))?;

            if !output.status.success() {
                return Err(format!(
                    "cp failed with exit code: {:?}",
                    output.status.code()
                ));
            }
        }

        Ok(())
    }

    fn run_rsync(
        src: &str,
        dest: &str,
        key_path: Option<&str>,
        port: Option<u16>,
    ) -> Result<(), String> {
        let mut args = vec!["-avz", "--progress"];
        let ssh_cmd_storage;

        if let Some(key) = key_path {
            args.push("-e");
            if let Some(p) = port {
                ssh_cmd_storage = format!("ssh -i {} -p {}", key, p);
            } else {
                ssh_cmd_storage = format!("ssh -i {}", key);
            }
            args.push(&ssh_cmd_storage);
        } else if let Some(p) = port {
            args.push("-e");
            ssh_cmd_storage = format!("ssh -p {}", p);
            args.push(&ssh_cmd_storage);
        }

        args.push(src);
        args.push(dest);

        let output = Command::new("rsync")
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| format!("Failed to execute rsync: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "rsync failed with exit code: {:?}",
                output.status.code()
            ));
        }

        Ok(())
    }

    fn run_scp(
        src: &str,
        dest: &str,
        key_path: Option<&str>,
        port: Option<u16>,
    ) -> Result<(), String> {
        let mut args = Vec::new();
        let port_str_storage;

        if let Some(key) = key_path {
            args.push("-i");
            args.push(key);
        }

        if let Some(p) = port {
            args.push("-P");
            port_str_storage = p.to_string();
            args.push(&port_str_storage);
        }

        args.push(src);
        args.push(dest);

        let output = Command::new("scp")
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| format!("Failed to execute scp: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "scp failed with exit code: {:?}",
                output.status.code()
            ));
        }

        Ok(())
    }

    fn list_remote(alias: &str, path: &str, config: &Config) -> Result<(), String> {
        let server = config.get_server(alias).ok_or_else(|| {
            format!(
                "Unknown server alias '{}'. Add it to your config first.",
                alias
            )
        })?;

        let remote_path = if path.is_empty() {
            server
                .default_remote_path
                .clone()
                .unwrap_or_else(|| format!("/home/{}", server.user))
        } else {
            path.to_string()
        };

        let mut args = Vec::new();
        let port_str_storage;

        if let Some(key) = &server.key_path {
            args.push("-i");
            args.push(key);
        }

        if let Some(p) = server.port {
            args.push("-p");
            port_str_storage = p.to_string();
            args.push(&port_str_storage);
        }

        let host_str = format!("{}@{}", server.user, server.host);
        let cmd_str = format!("ls -la {}", remote_path);
        args.push(&host_str);
        args.push(&cmd_str);

        let output = Command::new("ssh")
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| format!("Failed to execute ssh: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "ssh failed with exit code: {:?}",
                output.status.code()
            ));
        }

        Ok(())
    }
}

fn add_server(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "Adding a new server configuration".green().bold());

    let mut alias = String::new();
    print!("Server alias (e.g., 'gcp', 'aws-ec2'): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut alias)?;
    let alias = alias.trim().to_string();

    let mut host = String::new();
    print!("Host address (e.g., 'example.com', '10.0.0.1'): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut host)?;
    let host = host.trim().to_string();

    let mut user = String::new();
    print!("Username: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut user)?;
    let user = user.trim().to_string();

    let mut key_path = String::new();
    print!("SSH key path (optional, leave blank for none): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut key_path)?;
    let key_path = key_path.trim().to_string();
    let key_path = if key_path.is_empty() {
        None
    } else {
        Some(key_path)
    };

    let mut port_str = String::new();
    print!("SSH port (optional, default is 22): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut port_str)?;
    let port_str = port_str.trim();
    let port = if port_str.is_empty() {
        None
    } else {
        Some(port_str.parse::<u16>()?)
    };

    let mut default_path = String::new();
    print!("Default remote path (optional): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut default_path)?;
    let default_path = default_path.trim().to_string();
    let default_path = if default_path.is_empty() {
        None
    } else {
        Some(default_path)
    };

    let server_config = ServerConfig {
        host,
        user,
        key_path,
        port,
        default_remote_path: default_path,
    };

    config.servers.insert(alias.clone(), server_config);

    if config.default_server.is_none() {
        let mut set_default = String::new();
        print!("Set as default server? (y/n): ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut set_default)?;
        if set_default.trim().to_lowercase() == "y" {
            config.default_server = Some(alias);
        }
    }

    config.save()?;
    println!("{}", "Server configuration added successfully!".green());
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("xfer")
        .version("0.1.0")
        .author("Mutasim")
        .about("Simple file transfer tool")
        .subcommand(
            SubCommand::with_name("send")
                .about("Send a file or directory")
                .arg(
                    Arg::with_name("SOURCE")
                        .required(true)
                        .help("Source file or directory"),
                )
                .arg(
                    Arg::with_name("DESTINATION")
                        .required(true)
                        .help("Destination path"),
                ),
        )
        .subcommand(
            SubCommand::with_name("get")
                .about("Get a file or directory")
                .arg(
                    Arg::with_name("SOURCE")
                        .required(true)
                        .help("Source file or directory"),
                )
                .arg(
                    Arg::with_name("DESTINATION")
                        .required(true)
                        .help("Destination path"),
                ),
        )
        .subcommand(
            SubCommand::with_name("sync")
                .about("Sync directories")
                .arg(
                    Arg::with_name("SOURCE")
                        .required(true)
                        .help("Source directory"),
                )
                .arg(
                    Arg::with_name("DESTINATION")
                        .required(true)
                        .help("Destination directory"),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List files on remote server")
                .arg(
                    Arg::with_name("LOCATION")
                        .required(true)
                        .help("Location to list (alias:/path)"),
                ),
        )
        .subcommand(
            SubCommand::with_name("server")
                .about("Manage server configurations")
                .subcommand(SubCommand::with_name("add").about("Add a new server configuration"))
                .subcommand(SubCommand::with_name("list").about("List all server configurations")),
        )
        .get_matches();

    let mut config = Config::load()?;

    if config.servers.is_empty() {
        println!(
            "{}",
            "No server configurations found. Let's add one now.".yellow()
        );
        add_server(&mut config)?;
    }

    match matches.subcommand() {
        ("send", Some(sub_m)) => {
            let src = sub_m.value_of("SOURCE").unwrap();
            let dest = sub_m.value_of("DESTINATION").unwrap();

            println!("{} {} {} {}", "Sending".green(), src, "to".green(), dest);
            if let Err(e) = TransferEngine::send_file(src, dest, &config) {
                eprintln!("{}: {}", "Error".red().bold(), e);
                std::process::exit(1);
            }
        }
        ("get", Some(sub_m)) => {
            let src = sub_m.value_of("SOURCE").unwrap();
            let dest = sub_m.value_of("DESTINATION").unwrap();

            println!("{} {} {} {}", "Getting".green(), src, "to".green(), dest);
            if let Err(e) = TransferEngine::send_file(src, dest, &config) {
                eprintln!("{}: {}", "Error".red().bold(), e);
                std::process::exit(1);
            }
        }
        ("sync", Some(sub_m)) => {
            let src = sub_m.value_of("SOURCE").unwrap();
            let dest = sub_m.value_of("DESTINATION").unwrap();

            println!("{} {} {} {}", "Syncing".green(), src, "to".green(), dest);
            if let Err(e) = TransferEngine::send_file(src, dest, &config) {
                eprintln!("{}: {}", "Error".red().bold(), e);
                std::process::exit(1);
            }
        }
        ("list", Some(sub_m)) => {
            let location = sub_m.value_of("LOCATION").unwrap();
            let parts: Vec<&str> = location.splitn(2, ':').collect();

            if parts.len() != 2 {
                eprintln!(
                    "{}: Invalid location format. Use 'alias:/path'",
                    "Error".red().bold()
                );
                std::process::exit(1);
            }

            let alias = parts[0];
            let path = parts[1];

            println!("{} {} {}", "Listing".green(), path, "on".green());
            if let Err(e) = TransferEngine::list_remote(alias, path, &config) {
                eprintln!("{}: {}", "Error".red().bold(), e);
                std::process::exit(1);
            }
        }
        ("server", Some(sub_m)) => match sub_m.subcommand() {
            ("add", _) => {
                if let Err(e) = add_server(&mut config) {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    std::process::exit(1);
                }
            }
            ("list", _) => {
                println!("{}", "Configured Servers:".green().bold());
                for (alias, server) in &config.servers {
                    println!(
                        "  {} - {}@{}",
                        alias.yellow(),
                        server.user.cyan(),
                        server.host.cyan()
                    );
                    if let Some(default) = &config.default_server {
                        if default == alias {
                            println!("    {}", "DEFAULT".green());
                        }
                    }
                }
            }
            _ => unreachable!(),
        },
        _ => {
            println!("No command specified. Use --help for usage information.");
        }
    }

    Ok(())
}
