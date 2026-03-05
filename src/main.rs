use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process;
use toml_edit::{DocumentMut, value};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ROLE: &str = "main";

const HELP: &str = "\
phx-port — stable port assignments for your projects

USAGE:
    PORT=$(phx-port) iex -S mix phx.server
    PORT=$(phx-port) PORT_DEBUG=$(phx-port debug) my-app

    phx-port --list             List all registered projects and ports
    phx-port --register         Register the current directory for a new port
    phx-port --register debug   Register a named port role
    phx-port --delete <X>       Remove all ports (X = port number, directory name, or '.')
    phx-port --delete <X> debug Remove a specific port role

When piped (e.g. in a script), prints the port for the current directory,
auto-registering if needed. An optional positional argument specifies the
port role (default: main). Port 4000 is kept free.

Config: ~\\.config\\phx-ports.toml (override with PHX_PORT_CONFIG env var)
Home:   USERPROFILE or HOMEDRIVE+HOMEPATH (Windows), HOME (Linux/macOS)";

fn home_dir() -> PathBuf {
    #[cfg(target_family = "windows")]
    {
        if let Ok(profile) = env::var("USERPROFILE") {
            return PathBuf::from(profile);
        }
        if let (Ok(drive), Ok(path)) = (env::var("HOMEDRIVE"), env::var("HOMEPATH")) {
            return PathBuf::from(format!("{}{}", drive, path));
        }
        eprintln!("Error: could not determine home directory (USERPROFILE or HOMEDRIVE+HOMEPATH not set)");
        process::exit(1);
    }
    #[cfg(not(target_family = "windows"))]
    {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home);
        }
        eprintln!("Error: HOME environment variable not set");
        process::exit(1);
    }
}

fn config_path() -> PathBuf {
    if let Ok(custom) = env::var("PHX_PORT_CONFIG") {
        return PathBuf::from(custom);
    }
    home_dir().join(".config").join("phx-ports.toml")
}

fn read_config(path: &PathBuf) -> DocumentMut {
    let mut doc = if path.exists() {
        let content = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path.display(), e);
            process::exit(1);
        });
        content.parse::<DocumentMut>().unwrap_or_else(|e| {
            eprintln!("Error parsing {}: {}", path.display(), e);
            process::exit(1);
        })
    } else {
        "[ports]\n".parse::<DocumentMut>().unwrap()
    };

    // Migrate old flat format (dir = port) to new nested format (dir.role = port)
    ensure_ports_table(&mut doc);
    let old_entries: Vec<(String, i64)> = doc["ports"]
        .as_table()
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_integer().map(|p| (k.to_string(), p)))
                .collect()
        })
        .unwrap_or_default();
    if !old_entries.is_empty() {
        for (dir, port) in &old_entries {
            doc["ports"][dir] = toml_edit::table();
            doc["ports"][dir][DEFAULT_ROLE] = value(*port);
        }
        write_config(path, &doc);
    }

    doc
}

fn write_config(path: &PathBuf, doc: &DocumentMut) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| {
            eprintln!("Error creating {}: {}", parent.display(), e);
            process::exit(1);
        });
    }
    fs::write(path, doc.to_string()).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", path.display(), e);
        process::exit(1);
    });
}

fn cwd_string() -> String {
    env::current_dir()
        .unwrap_or_else(|e| {
            eprintln!("Error getting current directory: {}", e);
            process::exit(1);
        })
        .to_string_lossy()
        .to_string()
}

fn ensure_ports_table(doc: &mut DocumentMut) {
    if !doc.contains_table("ports") {
        doc["ports"] = toml_edit::table();
    }
}

fn next_port(doc: &DocumentMut) -> i64 {
    let mut used = BTreeSet::new();
    if let Some(table) = doc["ports"].as_table() {
        for (_, dir_value) in table.iter() {
            if let Some(dir_table) = dir_value.as_table() {
                for (_, port_value) in dir_table.iter() {
                    if let Some(port) = port_value.as_integer() {
                        used.insert(port);
                    }
                }
            }
        }
    }

    // Find the first gap starting from 4001
    let mut port = 4001;
    while used.contains(&port) {
        port += 1;
    }
    port
}

fn cmd_list(config: &PathBuf) {
    let doc = read_config(config);
    if let Some(table) = doc.get("ports").and_then(|v| v.as_table()) {
        let mut entries: Vec<(i64, String, String)> = Vec::new();
        for (dir, dir_value) in table.iter() {
            if let Some(dir_table) = dir_value.as_table() {
                for (role, port_value) in dir_table.iter() {
                    if let Some(port) = port_value.as_integer() {
                        entries.push((port, dir.to_string(), role.to_string()));
                    }
                }
            }
        }
        if entries.is_empty() {
            eprintln!("No ports registered. Use --register or PORT=$(phx-port) to add one.");
            return;
        }
        entries.sort_by_key(|(p, _, _)| *p);
        for (port, dir, role) in &entries {
            if role == DEFAULT_ROLE {
                println!("{:>5}  {}", port, dir);
            } else {
                println!("{:>5}  {} ({})", port, dir, role);
            }
        }
    } else {
        eprintln!("No ports registered.");
    }
}

fn cmd_register(config: &PathBuf, role: &str) {
    let cwd_str = cwd_string();
    let mut doc = read_config(config);
    ensure_ports_table(&mut doc);

    if let Some(port) = doc["ports"]
        .as_table()
        .and_then(|t| t.get(&cwd_str))
        .and_then(|v| v.as_table())
        .and_then(|t| t.get(role))
        .and_then(|v| v.as_integer())
    {
        if role == DEFAULT_ROLE {
            eprintln!("Already registered: {} → port {}", cwd_str, port);
        } else {
            eprintln!("Already registered: {} ({}) → port {}", cwd_str, role, port);
        }
        println!("{}", port);
        return;
    }

    let new_port = next_port(&doc);
    if doc["ports"]
        .as_table()
        .map_or(true, |t| !t.contains_key(&cwd_str))
    {
        doc["ports"][&cwd_str] = toml_edit::table();
    }
    doc["ports"][&cwd_str][role] = value(new_port);
    write_config(config, &doc);
    if role == DEFAULT_ROLE {
        eprintln!("Registered {} → port {}", cwd_str, new_port);
    } else {
        eprintln!("Registered {} ({}) → port {}", cwd_str, role, new_port);
    }
    println!("{}", new_port);
}

fn resolve_dir(doc: &DocumentMut, arg: &str) -> Option<String> {
    let table = doc["ports"].as_table()?;

    if arg == "." {
        let cwd = cwd_string();
        if table.contains_key(&cwd) {
            return Some(cwd);
        }
        eprintln!("Current directory is not registered: {}", cwd);
        return None;
    }

    // Try as directory name suffix
    let matches: Vec<&str> = table
        .iter()
        .map(|(k, _)| k)
        .filter(|k| k.ends_with(&format!("/{}", arg)))
        .collect();

    match matches.len() {
        0 => {
            eprintln!("No mapping found matching '{}'", arg);
            None
        }
        1 => Some(matches[0].to_string()),
        _ => {
            eprintln!("Ambiguous match for '{}'. Matching directories:", arg);
            for m in &matches {
                eprintln!("  {}", m);
            }
            None
        }
    }
}

fn cmd_delete(config: &PathBuf, arg: &str, role: Option<&str>) {
    let mut doc = read_config(config);
    ensure_ports_table(&mut doc);

    // Delete by port number: find the dir+role that owns this port
    if let Ok(port_num) = arg.parse::<i64>() {
        let mut found = None;
        if let Some(table) = doc["ports"].as_table() {
            for (dir, dir_value) in table.iter() {
                if let Some(dir_table) = dir_value.as_table() {
                    for (r, port_value) in dir_table.iter() {
                        if port_value.as_integer() == Some(port_num) {
                            found = Some((dir.to_string(), r.to_string()));
                        }
                    }
                }
            }
        }
        if let Some((dir, found_role)) = found {
            doc["ports"][&dir].as_table_mut().unwrap().remove(&found_role);
            if doc["ports"][&dir].as_table().map_or(true, |t| t.is_empty()) {
                doc["ports"].as_table_mut().unwrap().remove(&dir);
            }
            write_config(config, &doc);
            if found_role == DEFAULT_ROLE {
                eprintln!("Removed {} (was port {})", dir, port_num);
            } else {
                eprintln!("Removed {} ({}) (was port {})", dir, found_role, port_num);
            }
        } else {
            eprintln!("No mapping found for port {}", port_num);
            process::exit(1);
        }
        return;
    }

    // Resolve target to a directory key
    let key = match resolve_dir(&doc, arg) {
        Some(k) => k,
        None => process::exit(1),
    };

    if let Some(role) = role {
        // Delete a specific role
        if let Some(port) = doc["ports"]
            .as_table()
            .and_then(|t| t.get(&key))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get(role))
            .and_then(|v| v.as_integer())
        {
            doc["ports"][&key].as_table_mut().unwrap().remove(role);
            if doc["ports"][&key].as_table().map_or(true, |t| t.is_empty()) {
                doc["ports"].as_table_mut().unwrap().remove(&key);
            }
            write_config(config, &doc);
            if role == DEFAULT_ROLE {
                eprintln!("Removed {} (was port {})", key, port);
            } else {
                eprintln!("Removed {} ({}) (was port {})", key, role, port);
            }
        } else {
            eprintln!("No {} port registered for {}", role, key);
            process::exit(1);
        }
    } else {
        // Delete all roles for this directory
        let ports: Vec<(String, i64)> = doc["ports"]
            .as_table()
            .and_then(|t| t.get(&key))
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(r, v)| v.as_integer().map(|p| (r.to_string(), p)))
                    .collect()
            })
            .unwrap_or_default();

        doc["ports"].as_table_mut().unwrap().remove(&key);
        write_config(config, &doc);
        for (role, port) in &ports {
            if role == DEFAULT_ROLE {
                eprintln!("Removed {} (was port {})", key, port);
            } else {
                eprintln!("Removed {} ({}) (was port {})", key, role, port);
            }
        }
    }
}

fn cmd_port(config: &PathBuf, role: &str) {
    let cwd_str = cwd_string();
    let mut doc = read_config(config);
    ensure_ports_table(&mut doc);

    if let Some(port) = doc["ports"]
        .as_table()
        .and_then(|t| t.get(&cwd_str))
        .and_then(|v| v.as_table())
        .and_then(|t| t.get(role))
        .and_then(|v| v.as_integer())
    {
        println!("{}", port);
        return;
    }

    let new_port = next_port(&doc);
    if doc["ports"]
        .as_table()
        .map_or(true, |t| !t.contains_key(&cwd_str))
    {
        doc["ports"][&cwd_str] = toml_edit::table();
    }
    doc["ports"][&cwd_str][role] = value(new_port);
    write_config(config, &doc);
    if role == DEFAULT_ROLE {
        eprintln!("Registered {} → port {}", cwd_str, new_port);
    } else {
        eprintln!("Registered {} ({}) → port {}", cwd_str, role, new_port);
    }
    println!("{}", new_port);
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = config_path();

    match args.first().map(|s| s.as_str()) {
        Some("--version" | "-V") => {
            println!("phx-port {}", VERSION);
        }
        Some("--help" | "-h") => {
            println!("{}", HELP);
        }
        Some("--list" | "-l") => {
            cmd_list(&config);
        }
        Some("--register" | "-r") => {
            let role = args.get(1).map(|s| s.as_str()).unwrap_or(DEFAULT_ROLE);
            cmd_register(&config, role);
        }
        Some("--delete" | "-d") => {
            if let Some(target) = args.get(1) {
                let role = args.get(2).map(|s| s.as_str());
                cmd_delete(&config, target, role);
            } else {
                eprintln!("Usage: phx-port --delete <port|name|.> [role]");
                process::exit(1);
            }
        }
        Some(other) if other.starts_with('-') => {
            eprintln!("Unknown option: {}", other);
            eprintln!("{}", HELP);
            process::exit(1);
        }
        Some(role) => {
            // Non-flag argument is a port role name
            if std::io::stdout().is_terminal() {
                println!("{}", HELP);
            } else {
                cmd_port(&config, role);
            }
        }
        None => {
            // No arguments: if interactive, show help; if piped, print port
            if std::io::stdout().is_terminal() {
                println!("{}", HELP);
            } else {
                cmd_port(&config, DEFAULT_ROLE);
            }
        }
    }
}
