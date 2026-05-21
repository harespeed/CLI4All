# Raw Command Inventory

`data/raw/` is for command inventory collection, not for direct runtime translation.

Rules:

- `generated/` contains machine-generated inventories collected from local systems.
- `samples/` contains small committed sample inventories used for tooling, tests, and documentation.
- Raw inventory records are not executable CLI4ALL mappings by themselves.
- Raw inventories may include unsafe commands, admin tools, destructive commands, shell internals, aliases, and machine-specific helpers.
- New runtime mappings must be reviewed before they are promoted into `data/commands.source.json`.

Collector outputs:

- `tools/collectors/collect_macos_zsh.sh` -> `data/raw/generated/macos_zsh_commands.json`
- `tools/collectors/collect_gnu_linux.sh` -> `data/raw/generated/gnu_linux_commands.json`
- `tools/collectors/collect_powershell.ps1` -> `data/raw/generated/powershell_commands.json`
- `tools/collectors/collect_windows_cmd.ps1` -> `data/raw/generated/windows_cmd_commands.json`

Candidate generation:

- `tools/collectors/generate_command_candidates.py` reads `samples/` and any local `generated/` inventories.
- It writes `data/commands.candidates.json`.
- The candidates file is still review-only. It must not be merged blindly into `data/commands.source.json`.
