use std::collections::HashMap;

use serde_json::Value;

use crate::{
    herdr::{to_args, HerdrHost, SshHerdr},
    picker::PickerItem,
    ssh::ssh_output,
};

pub fn probe(target: &str) -> Result<Value, String> {
    let out = ssh_output(target, "command -v herdr >/dev/null && herdr status --json")?;
    serde_json::from_slice(&out).map_err(|err| format!("invalid remote probe json: {err}"))
}

pub fn list_terminals(target: &str) -> Result<Vec<PickerItem>, String> {
    let host = SshHerdr::new(target);
    let workspaces = host.json(&to_args(["workspace", "list"]))?;
    let panes = host.json(&to_args(["pane", "list"]))?;
    Ok(remote_terminal_items(target, &workspaces, &panes))
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
