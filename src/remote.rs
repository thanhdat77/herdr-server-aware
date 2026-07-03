use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::home,
    herdr::{to_args, HerdrHost, SshHerdr},
    picker::PickerItem,
    ssh::ssh_output,
};

pub fn probe(target: &str) -> Result<Value, String> {
    let out = ssh_output(target, "command -v herdr >/dev/null && herdr status --json")?;
    serde_json::from_slice(&out).map_err(|err| format!("invalid remote probe json: {err}"))
}

#[derive(Debug, Clone, Copy)]
pub struct ListOptions {
    pub cache: bool,
    pub refresh: bool,
    pub ttl_ms: u64,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            cache: false,
            refresh: false,
            ttl_ms: 10_000,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TerminalCache {
    target: String,
    updated_at_ms: u128,
    items: Vec<PickerItem>,
}

pub fn list_terminals_with_options(
    target: &str,
    options: ListOptions,
) -> Result<Vec<PickerItem>, String> {
    if options.cache && !options.refresh {
        if let Some(items) = read_fresh_cache(target, options.ttl_ms) {
            return Ok(items);
        }
    }
    let items = list_terminals_live(target)?;
    let _ = write_cache(target, &items);
    Ok(items)
}

fn list_terminals_live(target: &str) -> Result<Vec<PickerItem>, String> {
    let host = SshHerdr::new(target);
    let workspaces = host.json(&to_args(["workspace", "list"]))?;
    let panes = host.json(&to_args(["pane", "list"]))?;
    Ok(remote_terminal_items(target, &workspaces, &panes))
}

fn read_fresh_cache(target: &str, ttl_ms: u64) -> Option<Vec<PickerItem>> {
    let cache: TerminalCache =
        serde_json::from_str(&fs::read_to_string(cache_path(target)).ok()?).ok()?;
    if cache.target != target {
        return None;
    }
    (now_ms().checked_sub(cache.updated_at_ms)? <= u128::from(ttl_ms)).then_some(cache.items)
}

fn write_cache(target: &str, items: &[PickerItem]) -> Result<(), String> {
    let path = cache_path(target);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("failed to create cache dir: {err}"))?;
    }
    let cache = TerminalCache {
        target: target.into(),
        updated_at_ms: now_ms(),
        items: items.to_vec(),
    };
    let text = serde_json::to_string(&cache).map_err(|err| err.to_string())?;
    fs::write(&path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn cache_path(target: &str) -> PathBuf {
    cache_dir().join(format!("{}.json", cache_key(target)))
}

fn cache_dir() -> PathBuf {
    env::var("HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|_| env::var("XDG_STATE_HOME").map(PathBuf::from))
        .unwrap_or_else(|_| home().join(".local/state"))
        .join("herdr-server-aware/cache/remote-terminals")
}

fn cache_key(target: &str) -> String {
    target
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' | '@' => c,
            _ => '_',
        })
        .collect()
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub fn remote_terminal_items(target: &str, workspaces: &Value, panes: &Value) -> Vec<PickerItem> {
    let workspace_labels = workspace_labels(workspaces);
    panes
        .pointer("/result/panes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|pane| picker_item_for_pane(target, &workspace_labels, pane))
        .collect()
}

fn workspace_labels(workspaces: &Value) -> HashMap<String, String> {
    workspaces
        .pointer("/result/workspaces")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|workspace| {
            let id = workspace.get("workspace_id")?.as_str()?;
            let label = workspace.get("label").and_then(Value::as_str).unwrap_or(id);
            Some((id.to_string(), label.to_string()))
        })
        .collect()
}

fn picker_item_for_pane(
    target: &str,
    workspace_labels: &HashMap<String, String>,
    pane: &Value,
) -> Option<PickerItem> {
    let terminal_id = pane.get("terminal_id")?.as_str()?;
    let workspace_id = pane.get("workspace_id")?.as_str()?;
    let workspace = workspace_labels
        .get(workspace_id)
        .map(String::as_str)
        .unwrap_or(workspace_id);
    let pane_name = pane
        .get("label")
        .and_then(Value::as_str)
        .or_else(|| pane.get("agent").and_then(Value::as_str))
        .unwrap_or(terminal_id);
    let cwd = pane
        .get("foreground_cwd")
        .or_else(|| pane.get("cwd"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = pane
        .get("agent_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let pane_id = pane.get("pane_id").and_then(Value::as_str).unwrap_or("");

    Some(PickerItem {
        id: format!("{target}::{terminal_id}"),
        title: format!("{target} / {workspace} / {pane_name}"),
        subtitle: format!("{status} {pane_id} {cwd}").trim().to_string(),
        path: cwd.to_string(),
        kind: "remote-terminal".into(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn cache_key_is_filename_safe() {
        assert_eq!(cache_key("ssh://you@host:2222"), "ssh___you@host_2222");
    }

    #[test]
    fn maps_remote_panes_to_picker_items() {
        let workspaces = json!({
            "result": { "workspaces": [{"workspace_id": "w1", "label": "api"}] }
        });
        let panes = json!({
            "result": { "panes": [{
                "workspace_id": "w1",
                "pane_id": "w1:p1",
                "terminal_id": "term_abc",
                "label": "server",
                "agent_status": "idle",
                "foreground_cwd": "/srv/api"
            }] }
        });

        let items = remote_terminal_items("prod", &workspaces, &panes);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "prod::term_abc");
        assert_eq!(items[0].title, "prod / api / server");
        assert_eq!(items[0].kind, "remote-terminal");
        assert_eq!(items[0].path, "/srv/api");
    }
}
