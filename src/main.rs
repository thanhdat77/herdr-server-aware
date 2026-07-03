use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde_json::Value;

mod config;
mod herdr;
mod picker;
mod remote;
mod servers;
mod ssh;

use config::{ConnectMode, ServerMeta, META_FILE};
use herdr::{herdr_json, run_herdr};
use picker::PickerItem;
use servers::{collect_servers, ServerEntry};
use ssh::{ssh_connect_command, ssh_terminal_attach_command};

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
        Some("list") => list_servers(),
        Some("open") => open_from_args(),
        Some("init") => init_from_args(),
        Some("new-tab") => new_tab(),
        Some("reconnect") => reconnect_current(),
        Some("adopt") => adopt_current(),
        Some("probe") => probe_from_args(),
        Some("remote-list") => remote_list_from_args(),
        Some("attach-terminal") => attach_terminal_from_args(),
        Some("help") | Some("--help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}")),
    }
}

fn list_servers() -> Result<(), String> {
    let items: Vec<PickerItem> = collect_servers()
        .into_iter()
        .map(|server| PickerItem {
            id: server.name.clone(),
            title: server.name,
            subtitle: server.subtitle,
            path: server.path.display().to_string(),
            kind: "server".into(),
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string(&items).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn open_from_args() -> Result<(), String> {
    let id = env::args().nth(2).ok_or("open requires server id")?;
    let server = collect_servers()
        .into_iter()
        .find(|server| server.name == id)
        .ok_or_else(|| format!("unknown server: {id}"))?;
    open_server(&server)
}

fn open_server(server: &ServerEntry) -> Result<(), String> {
    match server.mode {
        ConnectMode::Ssh => open_ssh_server(server),
        ConnectMode::HerdrRemote => open_herdr_remote(server),
        ConnectMode::HerdrTerminal => Err(
            "herdr-terminal mode needs a terminal id; use remote-list then attach-terminal".into(),
        ),
    }
}

fn open_ssh_server(server: &ServerEntry) -> Result<(), String> {
    if let Some(workspace_id) = matching_server_workspace(&server.name)? {
        sync_server_workspace(&workspace_id, &found_for_server(server, ConnectMode::Ssh))?;
        run_herdr(["workspace", "focus", &workspace_id])?;
        let pane = current_pane()?;
        return run_connect_in_pane(&pane.pane_id, &server.target, ConnectMode::Ssh);
    }
    write_meta(
        &server.path,
        &server.target,
        Some(&server.name),
        ConnectMode::Ssh,
    )?;
    let json = herdr_json([
        "workspace",
        "create",
        "--cwd",
        &server.path.display().to_string(),
        "--label",
        &format!("server: {}", server.name),
        "--focus",
    ])?;
    if let Some(workspace_id) = json
        .pointer("/result/workspace/workspace_id")
        .and_then(Value::as_str)
    {
        let _ = run_herdr(["tab", "rename", &format!("{workspace_id}:t1"), "remote"]);
    }
    if let Some(pane_id) = json
        .pointer("/result/root_pane/pane_id")
        .and_then(Value::as_str)
    {
        run_connect_in_pane(pane_id, &server.target, ConnectMode::Ssh)?;
    }
    Ok(())
}

fn open_herdr_remote(server: &ServerEntry) -> Result<(), String> {
    write_meta(
        &server.path,
        &server.target,
        Some(&server.name),
        ConnectMode::HerdrRemote,
    )?;
    let pane = current_pane()?;
    let json = herdr_json([
        "tab",
        "create",
        "--workspace",
        &pane.workspace_id,
        "--cwd",
        &server.path.display().to_string(),
        "--label",
        &format!("remote: {}", server.name),
        "--focus",
    ])?;
    let pane_id = created_pane_id(&json)?;
    run_connect_in_pane(pane_id, &server.target, ConnectMode::HerdrRemote)
}

fn matching_server_workspace(name: &str) -> Result<Option<String>, String> {
    let want = format!("server: {name}").to_ascii_lowercase();
    let json = herdr_json(["workspace", "list"])?;
    let Some(workspaces) = json.pointer("/result/workspaces").and_then(Value::as_array) else {
        return Ok(None);
    };
    Ok(workspaces.iter().find_map(|ws| {
        let label = ws.get("label")?.as_str()?.to_ascii_lowercase();
        (label == want).then(|| ws.get("workspace_id")?.as_str().map(str::to_string))?
    }))
}

fn init_from_args() -> Result<(), String> {
    let mut dir = None;
    let mut target = None;
    let mut label = None;
    let mut mode = ConnectMode::Ssh;
    let mut args = env::args().skip(2);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--target" => target = args.next(),
            "--label" => label = args.next(),
            "--mode" => {
                mode = args
                    .next()
                    .map(|v| ConnectMode::parse(&v))
                    .ok_or("--mode requires value")?
            }
            other => return Err(format!("unknown init arg: {other}")),
        }
    }
    let dir = dir.ok_or("init requires --dir")?;
    let target = target.ok_or("init requires --target")?;
    write_meta(&dir, &target, label.as_deref(), mode)
}

fn new_tab() -> Result<(), String> {
    let pane = current_pane()?;
    let found = server_meta_for_pane(&pane);
    if let Some(found) = &found {
        sync_server_workspace(&pane.workspace_id, found)?;
    }
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
        let pane_id = created_pane_id(&json)?;
        run_connect_in_pane(
            pane_id,
            &found.meta.target,
            ConnectMode::parse(&found.meta.mode),
        )?;
    }
    Ok(())
}

fn reconnect_current() -> Result<(), String> {
    let pane = current_pane()?;
    let found = server_meta_for_pane(&pane).ok_or("no server metadata found for current pane")?;
    sync_server_workspace(&pane.workspace_id, &found)?;
    run_connect_in_pane(
        &pane.pane_id,
        &found.meta.target,
        ConnectMode::parse(&found.meta.mode),
    )
}

fn adopt_current() -> Result<(), String> {
    let pane = current_pane()?;
    let target = pane
        .cwd
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("cannot infer server target from cwd")?;
    write_meta(&pane.cwd, target, Some(target), ConnectMode::Ssh)?;
    let found = FoundMeta {
        dir: pane.cwd.clone(),
        meta: ServerMeta {
            target: target.into(),
            label: target.into(),
            mode: config::default_mode(),
        },
    };
    sync_server_workspace(&pane.workspace_id, &found)
}

fn probe_from_args() -> Result<(), String> {
    let target = target_from_arg(&env::args().nth(2).ok_or("probe requires server")?);
    let json = remote::probe(&target)?;
    println!(
        "{}",
        serde_json::to_string(&json).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn remote_list_from_args() -> Result<(), String> {
    let (target, options) = parse_remote_list_args(env::args().skip(2))?;
    let target = target_from_arg(&target);
    let items = remote::list_terminals_with_options(&target, options)?;
    println!(
        "{}",
        serde_json::to_string(&items).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn parse_remote_list_args<I>(args: I) -> Result<(String, remote::ListOptions), String>
where
    I: IntoIterator<Item = String>,
{
    let mut target = None;
    let mut options = remote::ListOptions::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cache" => options.cache = true,
            "--refresh" => options.refresh = true,
            "--ttl-ms" => {
                let value = args.next().ok_or("--ttl-ms requires value")?;
                options.ttl_ms = value
                    .parse()
                    .map_err(|_| format!("invalid --ttl-ms value: {value}"))?;
            }
            _ if target.is_none() => target = Some(arg),
            other => return Err(format!("unknown remote-list arg: {other}")),
        }
    }
    Ok((target.ok_or("remote-list requires server")?, options))
}

fn attach_terminal_from_args() -> Result<(), String> {
    let server_arg = env::args()
        .nth(2)
        .ok_or("attach-terminal requires server")?;
    let found = found_for_server_arg(&server_arg, ConnectMode::Ssh);
    let target = found.meta.target.clone();
    let terminal_id = env::args()
        .nth(3)
        .ok_or("attach-terminal requires terminal id")?;
    let pane = current_pane()?;
    sync_server_workspace(&pane.workspace_id, &found)?;
    let json = herdr_json([
        "tab",
        "create",
        "--workspace",
        &pane.workspace_id,
        "--cwd",
        &found.dir.display().to_string(),
        "--label",
        &format!("{}:{}", server_label(&found), terminal_id),
        "--focus",
    ])?;
    let pane_id = created_pane_id(&json)?;
    run_herdr([
        "pane",
        "run",
        pane_id,
        &ssh_terminal_attach_command(&target, &terminal_id, true),
    ])
}

fn target_from_arg(value: &str) -> String {
    found_for_server_arg(value, ConnectMode::Ssh).meta.target
}

fn found_for_server_arg(value: &str, mode: ConnectMode) -> FoundMeta {
    collect_servers()
        .into_iter()
        .find(|server| server.name == value)
        .map(|server| found_for_server(&server, mode))
        .unwrap_or_else(|| FoundMeta {
            dir: config::server_base_dir().join(value),
            meta: ServerMeta {
                target: value.into(),
                label: value.into(),
                mode: mode_name(mode).into(),
            },
        })
}

fn found_for_server(server: &ServerEntry, mode: ConnectMode) -> FoundMeta {
    FoundMeta {
        dir: server.path.clone(),
        meta: ServerMeta {
            target: server.target.clone(),
            label: server.name.clone(),
            mode: mode_name(mode).into(),
        },
    }
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
    let target = workspaces
        .iter()
        .find(|ws| ws.get("workspace_id").and_then(Value::as_str) == Some(workspace_id))?
        .get("label")?
        .as_str()?
        .trim()
        .strip_prefix("server:")?
        .trim();
    if target.is_empty() {
        return None;
    }
    let dir = config::server_base_dir().join(target);
    let _ = write_meta(&dir, target, Some(target), ConnectMode::Ssh);
    Some(FoundMeta {
        dir,
        meta: ServerMeta {
            target: target.into(),
            label: target.into(),
            mode: config::default_mode(),
        },
    })
}

fn infer_server_dir(cwd: &Path) -> Option<FoundMeta> {
    let base = config::server_base_dir();
    let rel = cwd.strip_prefix(&base).ok()?;
    let target = rel.components().next()?.as_os_str().to_str()?.to_string();
    if target.is_empty() {
        return None;
    }
    let dir = base.join(&target);
    let _ = write_meta(&dir, &target, Some(&target), ConnectMode::Ssh);
    Some(FoundMeta {
        dir,
        meta: ServerMeta {
            target: target.clone(),
            label: target,
            mode: config::default_mode(),
        },
    })
}

fn sync_server_workspace(workspace_id: &str, found: &FoundMeta) -> Result<(), String> {
    write_meta(
        &found.dir,
        &found.meta.target,
        Some(server_label(found)),
        ConnectMode::parse(&found.meta.mode),
    )?;
    run_herdr([
        "workspace",
        "rename",
        workspace_id,
        &format!("server: {}", server_label(found)),
    ])
}

fn server_label(found: &FoundMeta) -> &str {
    if found.meta.label.trim().is_empty() {
        &found.meta.target
    } else {
        &found.meta.label
    }
}

fn write_meta(
    dir: &Path,
    target: &str,
    label: Option<&str>,
    mode: ConnectMode,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
    let meta = ServerMeta {
        target: target.into(),
        label: label.unwrap_or(target).into(),
        mode: mode_name(mode).into(),
    };
    let text = toml::to_string_pretty(&meta).map_err(|err| err.to_string())?;
    fs::write(dir.join(META_FILE), text)
        .map_err(|err| format!("failed to write {}: {err}", dir.join(META_FILE).display()))
}

fn mode_name(mode: ConnectMode) -> &'static str {
    match mode {
        ConnectMode::Ssh => "ssh",
        ConnectMode::HerdrRemote => "herdr-remote",
        ConnectMode::HerdrTerminal => "herdr-terminal",
    }
}

fn run_connect_in_pane(pane_id: &str, target: &str, mode: ConnectMode) -> Result<(), String> {
    match mode {
        ConnectMode::Ssh => run_herdr(["pane", "run", pane_id, &ssh_connect_command(target)]),
        ConnectMode::HerdrRemote => run_herdr([
            "pane",
            "run",
            pane_id,
            &format!("exec herdr --remote {}", ssh::shell_quote(target)),
        ]),
        ConnectMode::HerdrTerminal => Err(
            "herdr-terminal mode needs a terminal id; use remote-list then attach-terminal".into(),
        ),
    }
}

fn created_pane_id(json: &Value) -> Result<&str, String> {
    json.pointer("/result/root_pane/pane_id")
        .or_else(|| json.pointer("/result/pane/pane_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "command did not return a pane id".into())
}

fn print_help() {
    println!(
        "herdr-server-aware\n\ncommands:\n  list\n  open SERVER\n  init --dir DIR --target TARGET [--label LABEL] [--mode ssh|herdr-remote|herdr-terminal]\n  new-tab\n  reconnect\n  adopt\n  probe SERVER\n  remote-list SERVER [--cache] [--refresh] [--ttl-ms N]\n  attach-terminal SERVER TERMINAL_ID"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_picker_server_base_dir() {
        let path = config::picker_server_base_dir("[servers]\nbase_dir = \"~/workspace/server\"\n")
            .unwrap();
        assert!(path.ends_with("workspace/server"));
    }

    #[test]
    fn mode_names_match_meta_values() {
        assert_eq!(mode_name(ConnectMode::Ssh), "ssh");
        assert_eq!(mode_name(ConnectMode::HerdrRemote), "herdr-remote");
        assert_eq!(mode_name(ConnectMode::HerdrTerminal), "herdr-terminal");
    }

    #[test]
    fn server_label_falls_back_to_target() {
        let found = FoundMeta {
            dir: PathBuf::from("/tmp/s1"),
            meta: ServerMeta {
                target: "s1".into(),
                label: "".into(),
                mode: "ssh".into(),
            },
        };
        assert_eq!(server_label(&found), "s1");
    }

    #[test]
    fn parses_remote_list_cache_flags() {
        let (target, options) = parse_remote_list_args([
            "nn".into(),
            "--cache".into(),
            "--ttl-ms".into(),
            "5000".into(),
        ])
        .unwrap();
        assert_eq!(target, "nn");
        assert!(options.cache);
        assert!(!options.refresh);
        assert_eq!(options.ttl_ms, 5000);
    }
}
