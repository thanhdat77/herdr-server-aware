# Herdr Server Aware

[![CI](https://github.com/thanhdat77/herdr-server-aware/actions/workflows/ci.yml/badge.svg)](https://github.com/thanhdat77/herdr-server-aware/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Small Herdr helper plugin for server workspaces.

It stores server identity in the workspace directory:

```toml
# ~/workspace/server/nn/.herdr-server.toml
target = "nn"
label = "nn"
mode = "ssh"
```

Then `new-server-tab` can create a new tab in the current workspace and reconnect with `autossh` automatically.

## Install locally

```bash
cargo build --release
herdr plugin link "$PWD"
```

## Picker integration

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
herdr-server-aware new-tab
herdr-server-aware reconnect
herdr-server-aware adopt
```

`new-tab` falls back to a normal Herdr tab when no `.herdr-server.toml`, server cwd, or `server: NAME` workspace label is found.
