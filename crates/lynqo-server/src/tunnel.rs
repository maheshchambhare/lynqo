use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{error, info};

fn find_cloudflared() -> Option<String> {
    let candidate_paths = [
        "/opt/homebrew/bin/cloudflared",
        "/usr/local/bin/cloudflared",
        "/usr/bin/cloudflared",
    ];

    for path in &candidate_paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn find_lt() -> Option<String> {
    let candidate_paths = [
        "/Users/mahesh/.nvm/versions/node/v22.23.1/bin/lt",
        "/opt/homebrew/bin/lt",
        "/usr/local/bin/lt",
        "/usr/bin/lt",
    ];

    for path in &candidate_paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    None
}

fn find_node() -> String {
    let candidate_paths = [
        "/Users/mahesh/.nvm/versions/node/v22.23.1/bin/node",
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ];

    for path in &candidate_paths {
        if Path::new(path).exists() {
            return path.to_string();
        }
    }

    "node".to_string()
}

pub fn start_auto_tunnel(db: lynqo_db::Database, port: u16) {
    tokio::spawn(async move {
        let (cmd, args) = if let Some(cf_path) = find_cloudflared() {
            info!("🚀 Using Cloudflare Quick Tunnel binary at {cf_path}");
            (cf_path, vec!["tunnel".to_string(), "--url".to_string(), format!("http://localhost:{port}")])
        } else if let Some(lt_path) = find_lt() {
            let node_cmd = find_node();
            info!("Using localtunnel via {node_cmd} {lt_path}");
            (node_cmd, vec![lt_path, "--port".to_string(), port.to_string(), "--subdomain".to_string(), "lynqo-share".to_string()])
        } else {
            let node_cmd = find_node();
            let lt_path = find_lt().unwrap_or_else(|| "lt".to_string());
            (node_cmd, vec![lt_path, "--port".to_string(), port.to_string()])
        };

        info!("Launching public reverse tunnel via {cmd} {:?}...", args);

        let mut child = match Command::new(&cmd)
            .args(&args)
            .env("FORCE_COLOR", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to spawn tunnel process via {cmd}: {e}");
                return;
            }
        };

        let db_stdout = db.clone();
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[Tunnel STDOUT]: {line}");
                    if let Some(url) = parse_tunnel_url(&line) {
                        info!("🚀 Public HTTPS Reverse Tunnel Active: {url}");
                        let _ = db_stdout.set_setting("public_domain", &url).await;
                    }
                }
            });
        }

        let db_stderr = db.clone();
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[Tunnel STDERR]: {line}");
                    if let Some(url) = parse_tunnel_url(&line) {
                        info!("🚀 Public HTTPS Reverse Tunnel Active: {url}");
                        let _ = db_stderr.set_setting("public_domain", &url).await;
                    }
                }
            });
        }
    });
}

fn parse_tunnel_url(line: &str) -> Option<String> {
    for word in line.split_whitespace() {
        let clean = word.trim_matches(|c: char| c == '"' || c == '\'' || c == '<' || c == '>' || c == '.' || c == '|' || c == ',');
        if clean.contains(".trycloudflare.com") || clean.contains(".loca.lt") || clean.contains(".lhr.life") || clean.contains(".localhost.run") {
            if clean.starts_with("https://") {
                return Some(clean.to_string());
            } else {
                return Some(format!("https://{}", clean));
            }
        }
    }
    None
}
