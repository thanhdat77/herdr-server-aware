use std::{env, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const META_FILE: &str = ".herdr-server.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerMeta {
    pub target: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub servers: ServersConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServersConfig {
    #[serde(default = "default_base_dir")]
    pub base_dir: String,
    #[serde(default = "yes")]
    pub ssh_config: bool,
    #[serde(default)]
    pub entries: Vec<ServerEntryConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerEntryConfig {
    pub name: String,
    pub host: Option<String>,
    pub user: Option<String>,
    pub target: Option<String>,
    pub mode: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectMode {
    Ssh,
    HerdrRemote,
    HerdrTerminal,
}

impl ConnectMode {
    pub fn parse(value: &str) -> Self {
        match value {
            "herdr-remote" => Self::HerdrRemote,
            "herdr-terminal" => Self::HerdrTerminal,
            _ => Self::Ssh,
        }
    }
}

pub fn load_config() -> Config {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .or_else(|| {
            fs::read_to_string(picker_config_path())
                .ok()
                .and_then(|text| toml::from_str(&text).ok())
        })
        .unwrap_or_default()
}

pub fn config_path() -> PathBuf {
    config_home().join("herdr/plugins/config/herdr-server-aware/config.toml")
}

pub fn picker_config_path() -> PathBuf {
    config_home().join("herdr/plugins/config/herdr-picker-plus/config.toml")
}

pub fn config_home() -> PathBuf {
    env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".config"))
}

pub fn server_base_dir() -> PathBuf {
    expand_home(&load_config().servers.base_dir)
}

#[cfg(test)]
pub fn picker_server_base_dir(text: &str) -> Option<PathBuf> {
    let value = text.parse::<toml::Value>().ok()?;
    let raw = value.get("servers")?.get("base_dir")?.as_str()?;
    Some(expand_home(raw))
}

pub fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        home()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(value)
    }
}

pub fn home() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

pub fn default_mode() -> String {
    "ssh".into()
}

fn default_base_dir() -> String {
    "~/workspace/server".into()
}

fn yes() -> bool {
    true
}

impl Default for ServersConfig {
    fn default() -> Self {
        Self {
            base_dir: default_base_dir(),
            ssh_config: true,
            entries: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_mode() {
        assert_eq!(ConnectMode::parse("herdr-remote"), ConnectMode::HerdrRemote);
        assert_eq!(
            ConnectMode::parse("herdr-terminal"),
            ConnectMode::HerdrTerminal
        );
        assert_eq!(ConnectMode::parse("unknown"), ConnectMode::Ssh);
    }
}
