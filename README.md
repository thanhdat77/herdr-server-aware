# Herdr Server Aware

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
herdr-server-aware init --dir ~/workspace/server/nn --target nn --label nn
herdr-server-aware new-tab
herdr-server-aware reconnect
herdr-server-aware adopt
```

`new-tab` falls back to a normal Herdr tab when no `.herdr-server.toml`, server cwd, or `server: NAME` workspace label is found.
