# Herdr Server Aware

[![CI](https://github.com/thanhdat77/herdr-server-aware/actions/workflows/ci.yml/badge.svg)](https://github.com/thanhdat77/herdr-server-aware/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Make every remote server feel like a local workspace.

Herdr Server Aware keeps your local Herdr workflow while making remote servers easy to open, reconnect, and attach to. Use plain SSH for quick shells, `herdr --remote` for full remote sessions, or attach one remote Herdr terminal back into your current local workspace.

## Why not just SSH?

Normal SSH opens a fresh shell. This plugin remembers server identity, opens smart server tabs, and can attach to a terminal that is already alive inside remote Herdr.

Compared with plain SSH:

- `new-tab` reconnects to the same server automatically.
- `remote-list` discovers Herdr terminals already running on the server.
- `attach-terminal` pulls one persistent remote terminal into your local workspace.

Compared with full `herdr --remote`:

- you keep your local workspace as the main workspace.
- you attach only the remote terminal you need, not the whole remote Herdr UI.
- trade-off: local Herdr sees it as one SSH-backed pane, not native remote panes/tabs.

It stores server identity in the workspace directory:

```toml
# ~/workspace/server/nn/.herdr-server.toml
target = "nn"
label = "nn"
mode = "ssh"
```

Then `new-server-tab`, `reconnect`, and `attach-terminal` keep the workspace synced to the `server: NAME` convention so restored workspaces can be recognized later.

## Install locally

```bash
cargo build --release
herdr plugin link "$PWD"
```

## Search and picker integration

`herdr-server-aware` prints picker-friendly JSON, so any search UI can use it:

```bash
herdr-server-aware list              # local server entries
herdr-server-aware remote-list nn           # live remote Herdr terminals on server nn
herdr-server-aware remote-list nn --cache   # use fresh cached result when available
```

Each item has this shape:

```json
{
  "id": "nn::term_abc123",
  "title": "nn / api / pi",
  "subtitle": "idle w1:p1 /srv/api",
  "path": "/srv/api",
  "kind": "remote-terminal"
}
```

### herdr-picker-plus: servers

Add this to `herdr-picker-plus` config so servers still appear under `Ctrl-S`:

```toml
[[integrations]]
id = "server-aware"
label = "server"
enabled = true
collect = "herdr-server-aware list"
open = "herdr-server-aware open {{id}}"
notify_success = false
notify_error = true
```

### herdr-picker-plus: remote Herdr terminals

For one known server:

```toml
[[integrations]]
id = "server-aware-terminals-nn"
label = "nn terminals"
enabled = true
collect = "herdr-server-aware remote-list nn --cache --ttl-ms 10000"
open = "bash -lc 'id=\"$1\"; server=${id%%::*}; term=${id#*::}; herdr-server-aware attach-terminal \"$server\" \"$term\"' -- {{id}}"
notify_success = false
notify_error = true
```

This searches remote Herdr panes, then opens only the selected terminal in your current local workspace.

## Config

```toml
[servers]
base_dir = "~/workspace/server"
ssh_config = true

# [[servers.entries]]
# name = "prod-api"
# host = "10.0.0.5"
# user = "ubuntu"
# tags = ["prod", "api"]
#
# [[servers.entries]]
# name = "prod-shortcut"
# target = "prod-api"
# mode = "ssh" # ssh | herdr-remote | herdr-terminal
# tags = ["prod"]
```

## Keybinding

```toml
[[keys.command]]
key = "prefix+c"
type = "plugin_action"
command = "herdr-server-aware.new-server-tab"
description = "smart server tab"
```

## Commands

```bash
herdr-server-aware list
herdr-server-aware open nn
herdr-server-aware init --dir ~/workspace/server/nn --target nn --label nn
herdr-server-aware init --dir ~/workspace/server/nn --target nn --mode herdr-remote
herdr-server-aware new-tab
herdr-server-aware reconnect
herdr-server-aware adopt
herdr-server-aware probe nn
herdr-server-aware remote-list nn
herdr-server-aware attach-terminal nn term_abc123
```

`new-tab` falls back to a normal Herdr tab when no `.herdr-server.toml`, server cwd, or `server: NAME` workspace label is found. `open SERVER` focuses an existing `server: SERVER` workspace and reconnects its focused pane.

## Remote Herdr terminal attach

For a server that already runs Herdr, list remote terminals:

```bash
herdr-server-aware remote-list nn
```

For picker/search integrations, use a short cache so repeated searches do not SSH on every keystroke:

```bash
herdr-server-aware remote-list nn --cache --ttl-ms 10000
herdr-server-aware remote-list nn --refresh
```

`remote-list` is live by default and writes the cache after a successful fetch. `--cache` returns a fresh cache entry when available; `--refresh` forces a live fetch.

Then attach one remote Herdr terminal into the current local workspace:

```bash
herdr-server-aware attach-terminal nn term_abc123
```

This creates a local tab that runs:

```bash
ssh -tt nn 'herdr terminal attach term_abc123 --takeover'
```

Use `mode = "herdr-remote"` when you want a tab to run full `herdr --remote nn` instead of plain SSH.
