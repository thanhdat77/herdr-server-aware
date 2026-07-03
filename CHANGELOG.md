# Changelog

## [Unreleased]

### Added
- Optional `remote-list` cache with `--cache`, `--refresh`, and `--ttl-ms` for picker/search integrations.
- Agent project docs under `.pi/docs/`.

### Changed
- Server workflows now sync workspace label/metadata to the `server: NAME` convention before reconnecting or attaching terminals.

## [0.2.0] - 2026-07-02

### Added
- Remote Herdr terminal discovery with `remote-list SERVER`.
- Single terminal attach with `attach-terminal SERVER TERMINAL_ID`.
- `probe SERVER` command for checking remote Herdr availability.
- Modular host/SSH/server/picker structure for future connection modes.
- Search and picker integration docs for remote Herdr terminals.

## [0.1.0] - 2026-07-02

### Added
- `list` and `open SERVER` commands for Herdr Picker Plus command/JSON integration.
- Server config support via `[servers]`, `ssh_config`, and `[[servers.entries]]`.
- Server metadata file support via `.herdr-server.toml`.
- `new-server-tab` plugin action: create a new tab and auto-connect when inside a server workspace.
- `reconnect-current` plugin action: reconnect the current pane to the remembered server target.
- `adopt-current` plugin action: write metadata for the current directory.
- Fallback server-dir inference from Herdr Picker Plus `[servers].base_dir`.
