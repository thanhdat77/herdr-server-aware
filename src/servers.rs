use std::{fs, path::Path, path::PathBuf};

use crate::config::{home, load_config, Config, ConnectMode};

#[derive(Debug, Clone)]
pub struct ServerEntry {
    pub name: String,
    pub target: String,
    pub path: PathBuf,
    pub subtitle: String,
    pub mode: ConnectMode,
}

pub fn collect_servers() -> Vec<ServerEntry> {
    let config = load_config();
    collect_servers_from_config(&config)
}

fn collect_servers_from_config(config: &Config) -> Vec<ServerEntry> {
    let base_dir = crate::config::expand_home(&config.servers.base_dir);
    let mut servers = Vec::new();
    if config.servers.ssh_config {
        let path = home().join(".ssh/config");
        if let Ok(text) = fs::read_to_string(path) {
            servers.extend(ssh_config_hosts(&text).into_iter().map(|host| {
                server_entry(
                    &host.name,
                    host.hostname.as_deref(),
                    host.user.as_deref(),
                    Some(&host.name),
                    &[],
                    &base_dir,
                    ConnectMode::Ssh,
                )
            }));
        }
    }
    servers.extend(config.servers.entries.iter().map(|server| {
        server_entry(
            &server.name,
            server.host.as_deref(),
            server.user.as_deref(),
            server.target.as_deref(),
            &server.tags,
            &base_dir,
            server
                .mode
                .as_deref()
                .map(ConnectMode::parse)
                .unwrap_or(ConnectMode::Ssh),
        )
    }));
    servers
}

fn server_entry(
    name: &str,
    host: Option<&str>,
    user: Option<&str>,
    target_override: Option<&str>,
    tags: &[String],
    base_dir: &Path,
    mode: ConnectMode,
) -> ServerEntry {
    let target = target_override
        .map(str::to_string)
        .unwrap_or_else(|| match (user, host) {
            (Some(user), Some(host)) => format!("{user}@{host}"),
            (_, Some(host)) => host.to_string(),
            _ => name.to_string(),
        });
    let mut search = vec![target.clone()];
    if let Some(host) = host {
        search.push(host.into());
    }
    if let Some(user) = user {
        search.push(user.into());
    }
    search.extend(tags.iter().cloned());
    let prefix = match mode {
        ConnectMode::Ssh => "autossh/ssh",
        ConnectMode::HerdrRemote => "herdr --remote",
        ConnectMode::HerdrTerminal => "remote Herdr terminal",
    };
    ServerEntry {
        name: name.into(),
        target: target.clone(),
        path: base_dir.join(name),
        subtitle: format!("{prefix} {}", search.join(" ")),
        mode,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshHost {
    pub name: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
}

pub fn ssh_config_hosts(text: &str) -> Vec<SshHost> {
    let mut out = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut hostname: Option<String> = None;
    let mut user: Option<String> = None;

    for line in text.lines().map(clean_ssh_config_line) {
        let Some((key, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let key = key.to_ascii_lowercase();
        let value = value.trim();
        if key == "host" {
            flush_ssh_hosts(&mut out, &names, hostname.take(), user.take());
            names = value
                .split_whitespace()
                .filter(|name| !name.contains(['*', '?', '!']))
                .map(str::to_string)
                .collect();
        } else if key == "hostname" {
            hostname = Some(value.into());
        } else if key == "user" {
            user = Some(value.into());
        }
    }
    flush_ssh_hosts(&mut out, &names, hostname, user);
    out
}

fn clean_ssh_config_line(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

fn flush_ssh_hosts(
    out: &mut Vec<SshHost>,
    names: &[String],
    hostname: Option<String>,
    user: Option<String>,
) {
    for name in names {
        out.push(SshHost {
            name: name.clone(),
            hostname: hostname.clone(),
            user: user.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_config_hosts() {
        let hosts = ssh_config_hosts(
            r#"
Host prod prod-short !skip *.wild
  HostName 10.0.0.5
  User ubuntu
Host dev # comment
  HostName dev.local
"#,
        );
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0].name, "prod");
        assert_eq!(hosts[0].hostname.as_deref(), Some("10.0.0.5"));
        assert_eq!(hosts[0].user.as_deref(), Some("ubuntu"));
        assert_eq!(hosts[2].name, "dev");
        assert_eq!(hosts[2].hostname.as_deref(), Some("dev.local"));
    }
}
