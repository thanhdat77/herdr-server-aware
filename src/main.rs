use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const META_FILE: &str = ".herdr-server.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ServerMeta {
    target: String,
    #[serde(default)]
    label: String,
    #[serde(default = "default_mode")]
    mode: String,
}

#[derive(Debug)]
struct FoundMeta {
    dir: PathBuf,
    meta: ServerMeta,
}

#[derive(Debug)]
struct PaneContext {
    workspace_id: String,
    pane_id: String,
    cwd: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    match env::args().nth(1).as_deref() {
        Some("init") => init_from_args(),
        Some("new-tab") => new_tab(),
        Some("reconnect") => reconnect_current(),
        Some("adopt") => adopt_current(),
        Some("help") | Some("--help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}")),
    }
}

fn init_from_args() -> Result<(), String> {
    let mut dir = None;
    let mut target = None;
    let mut label = None;
    let mut args = env::args().skip(2);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--target" => target = args.next(),
            "--label" => label = args.next(),
            other => return Err(format!("unknown init arg: {other}")),
        }
    }
    let dir = dir.ok_or("init requires --dir")?;
    let target = target.ok_or("init requires --target")?;
    write_meta(&dir, &target, label.as_deref())
}

fn new_tab() -> Result<(), String> {
    let pane = current_pane()?;
    let found = server_meta_for_pane(&pane);
    let cwd = found.as_ref().map(|m| m.dir.as_path()).unwrap_or(&pane.cwd);
    let mut args = vec![
        "tab".into(),
        "create".into(),
        "--workspace".into(),
        pane.workspace_id,
        "--cwd".into(),
        cwd.display().to_string(),
        "--focus".into(),
    ];
    if found.is_some() {
        args.push("--label".into());
        args.push("remote".into());
    }

    let json = herdr_json(args)?;
    if let Some(found) = found {
        let pane_id = json
            .pointer("/result/root_pane/pane_id")
            .or_else(|| json.pointer("/result/pane/pane_id"))
            .and_then(Value::as_str)
            .ok_or("tab create did not return a pane id")?;
        run_ssh_in_pane(pane_id, &found.meta.target)?;
    }
    Ok(())
}

fn reconnect_current() -> Result<(), String> {
    let pane = current_pane()?;
    let found = server_meta_for_pane(&pane).ok_or("no server metadata found for current pane")?;
    run_ssh_in_pane(&pane.pane_id, &found.meta.target)
}

fn adopt_current() -> Result<(), String> {
    let pane = current_pane()?;
    let target = pane
        .cwd
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("cannot infer server target from cwd")?;
    write_meta(&pane.cwd, target, Some(target))
}

fn current_pane() -> Result<PaneContext, String> {
    let json = herdr_json(["pane", "current"])?;
    let pane = json.pointer("/result/pane").ok_or("missing current pane")?;
    let workspace_id = pane
        .get("workspace_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("current pane has no workspace_id")?
        .to_string();
    let pane_id = pane
        .get("pane_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("current pane has no pane_id")?
        .to_string();
    let cwd = pane
        .get("foreground_cwd")
        .or_else(|| pane.get("cwd"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("current pane has no cwd")?;
    Ok(PaneContext {
        workspace_id,
        pane_id,
        cwd: PathBuf::from(cwd),
    })
}

fn server_meta_for_pane(pane: &PaneContext) -> Option<FoundMeta> {
    find_meta(&pane.cwd)
        .or_else(|| infer_server_dir(&pane.cwd))
        .or_else(|| infer_server_workspace(&pane.workspace_id))
}

fn find_meta(start: &Path) -> Option<FoundMeta> {
    for dir in start.ancestors() {
        let path = dir.join(META_FILE);
        let text = fs::read_to_string(&path).ok()?;
        let meta = toml::from_str::<ServerMeta>(&text).ok()?;
        if !meta.target.trim().is_empty() {
            return Some(FoundMeta {
                dir: dir.to_path_buf(),
                meta,
            });
        }
    }
    None
}

fn infer_server_workspace(workspace_id: &str) -> Option<FoundMeta> {
    let json = herdr_json(["workspace", "list"]).ok()?;
    let workspaces = json.pointer("/result/workspaces")?.as_array()?;
    let label = workspaces
        .iter()
        .find(|ws| ws.get("workspace_id").and_then(Value::as_str) == Some(workspace_id))?
        .get("label")?
        .as_str()?;
    let target = label.trim().strip_prefix("server:")?.trim();
    if target.is_empty() {
        return None;
    }
    let dir = server_base_dir().join(target);
    let _ = write_meta(&dir, target, Some(target));
    Some(FoundMeta {
        dir,
        meta: ServerMeta {
            target: target.into(),
            label: target.into(),
            mode: default_mode(),
        },
    })
}

fn infer_server_dir(cwd: &Path) -> Option<FoundMeta> {
    let base = server_base_dir();
    let rel = cwd.strip_prefix(&base).ok()?;
    let target = rel.components().next()?.as_os_str().to_str()?.to_string();
    if target.is_empty() {
        return None;
    }
    let dir = base.join(&target);
    let _ = write_meta(&dir, &target, Some(&target));
    Some(FoundMeta {
        dir,
        meta: ServerMeta {
            target: target.clone(),
            label: target,
            mode: default_mode(),
        },
    })
}

fn write_meta(dir: &Path, target: &str, label: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
    let meta = ServerMeta {
        target: target.into(),
        label: label.unwrap_or(target).into(),
        mode: default_mode(),
    };
    let text = toml::to_string_pretty(&meta).map_err(|err| err.to_string())?;
    fs::write(dir.join(META_FILE), text)
        .map_err(|err| format!("failed to write {}: {err}", dir.join(META_FILE).display()))
}

fn run_ssh_in_pane(pane_id: &str, target: &str) -> Result<(), String> {
    run_herdr([
        "pane",
        "run",
        pane_id,
        &ssh_connect_command(target),
    ])
}

fn ssh_connect_command(target: &str) -> String {
    let target = shell_quote(target);
    format!(
        "if command -v autossh >/dev/null 2>&1; then exec autossh -M 0 -o ServerAliveInterval=10 -o ServerAliveCountMax=3 -o TCPKeepAlive=yes {target}; else exec ssh -o ServerAliveInterval=10 -o ServerAliveCountMax=3 -o TCPKeepAlive=yes {target}; fi"
    )
}

fn herdr_json<I, S>(args: I) -> Result<Value, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let out = Command::new("herdr")
        .args(args.into_iter().map(|s| s.as_ref().to_string()))
        .output()
        .map_err(|err| format!("failed to run herdr: {err}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|err| format!("invalid herdr json: {err}"))
}

fn run_herdr<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let out = Command::new("herdr")
        .args(args.into_iter().map(|s| s.as_ref().to_string()))
        .output()
        .map_err(|err| format!("failed to run herdr: {err}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn server_base_dir() -> PathBuf {
    let config = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".config"))
        .join("herdr/plugins/config/herdr-picker-plus/config.toml");
    fs::read_to_string(config)
        .ok()
        .and_then(|text| picker_server_base_dir(&text))
        .unwrap_or_else(|| home().join("workspace/server"))
}

fn picker_server_base_dir(text: &str) -> Option<PathBuf> {
    let value = text.parse::<toml::Value>().ok()?;
    let raw = value.get("servers")?.get("base_dir")?.as_str()?;
    Some(expand_home(raw))
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        home()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(value)
    }
}

fn home() -> PathBuf {
    env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn default_mode() -> String {
    "ssh".into()
}

fn print_help() {
    println!(
        "herdr-server-aware\n\ncommands:\n  init --dir DIR --target TARGET [--label LABEL]\n  new-tab\n  reconnect\n  adopt"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_picker_server_base_dir() {
        let path = picker_server_base_dir("[servers]\nbase_dir = \"~/workspace/server\"\n").unwrap();
        assert!(path.ends_with("workspace/server"));
    }

    #[test]
    fn ssh_command_prefers_autossh() {
        let cmd = ssh_connect_command("prod-api");
        assert!(cmd.contains("autossh -M 0"));
        assert!(cmd.contains("else exec ssh"));
        assert!(cmd.contains("'prod-api'"));
    }

    #[test]
    fn shell_quote_handles_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
