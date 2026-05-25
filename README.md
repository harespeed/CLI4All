# CLI4ALL v0.1

CLI4ALL lets you type the command you remember and runs the correct version for your current OS.

It remains deterministic and offline. There are no LLMs, no online APIs, and no blind shell passthrough.

## Product Direction

CLI4ALL is becoming an independent desktop terminal application. The current repository includes:

- a Rust CLI core
- `cli4all shell` as the Phase 1 terminal prototype
- a PTY-backed desktop app in `desktop/` using Tauri, React, TypeScript, and xterm.js

In shell mode and in the desktop backend, CLI4ALL:

- detects the command source platform
- matches a known intent from the command store
- translates that intent to a native command for the current OS
- runs only the translated native command after safety checks
- prints the real stdout, stderr, and exit status

The command catalog is intent-based rather than literal-string based. A single intent such as
`list_files` can include Windows CMD aliases like `dir`, PowerShell aliases like
`Get-ChildItem`, and macOS/Linux aliases like `ls`, while still translating to the correct native
target command for the current platform.

## Safety Model

CLI4ALL does not blindly execute user input. In interactive shell mode and in the desktop backend, CLI4ALL executes only known translated native commands after detection, translation, and safety checks.

Safety rules:

- Foreign commands are never executed directly.
- Low-risk known translated commands may be executed automatically.
- Medium-risk and high-risk commands require confirmation.
- Destructive commands are blocked by default.
- Unknown commands are not auto-executed in Phase 1.
- CLI4ALL always prints the translated command before execution.

## Commands

```bash
cargo run -- check "ipconfig"
cargo run -- translate "dir" --to ubuntu
cargo run -- explain "chmod -R 777 ."
cargo run -- risk "rm -rf /"
cargo run -- fix "command not found: ipconfig"
cargo run -- shell
cargo run -- build-index --input data/commands.source.json --index data/commands.c4idx --data data/commands.c4dat
```

Existing helper commands remain available. `translate` now accepts `ubuntu`, `macos`, or `windows`.

## Shell Mode

Start the interactive prototype:

```bash
cli4all shell
```

Prompt examples:

- `cli4all-macos>`
- `cli4all-ubuntu>`
- `cli4all-windows>`

Exit with `exit` or `quit`.

### Demo: macOS

```text
cli4all shell
cli4all-macos> ipconfig

Original command: ipconfig
Detected source: Windows CMD
Current OS: macOS
Matched intent: show_ip_config
Translated command: ifconfig
Risk level: low
Running: ifconfig
```

### Demo: Ubuntu

```text
cli4all-ubuntu> open .

Original command: open .
Detected source: macOS
Current OS: Ubuntu
Matched intent: open_current_directory
Translated command: xdg-open .
Risk level: low
Running: xdg-open .
```

Linux distro detection is not implemented yet. Phase 1 maps `std::env::consts::OS == "linux"` to the `ubuntu` command set.

### Demo: Windows

```text
cli4all-windows> ls -la

Original command: ls -la
Detected source: Ubuntu
Current OS: Windows
Matched intent: list_all_files
Translated command: Get-ChildItem -Force
Risk level: low
Running: Get-ChildItem -Force
```

## Rule Files

- `data/commands.source.json` is the source catalog. Edit this file when you add or refine mappings.
- `data/commands.yaml` is a readable YAML mirror of the source catalog for review and documentation.
- `data/commands.c4idx` is a generated read-only B+ Tree index over normalized lookup keys.
- `data/commands.c4dat` is a generated flat data file containing serialized command records.
- `data/commands.candidates.json` is a review-only candidate file generated from raw inventories.
- `data/risks.yaml` stores regex-driven safety rules.

Rebuild the runtime store after editing the source catalog:

```bash
cargo run -- build-index --input data/commands.source.json --index data/commands.c4idx --data data/commands.c4dat
```

Current expansion priority is macOS <-> Windows command coverage, with Ubuntu/Linux mappings
continuing to grow alongside them.

Catalog expansion strategy:

- reviewed intent records are promoted deliberately from raw inventories and candidates
- raw inventories are never dumped directly into the runtime store
- every mapping must preserve arguments safely enough to survive cross-platform translation
- safety stays conservative: read-only commands stay low risk, filesystem writes and package/script execution require confirmation, destructive patterns stay blocked

High-frequency reviewed groups now include:

- file and directory search: `search_text_recursive`, `find_by_name`, `count_lines_words_chars`
- file inspection: `head_file`, `tail_file`, `follow_file`, `file_hash`
- network diagnostics: `trace_route`, `dns_lookup`, `http_head`, `download_to_file`, `check_port`
- processes and ports: `find_process_by_name`, `list_listening_ports`, `process_by_port`
- archives: `create_zip`, `extract_zip`, `create_tar_gz`, `extract_tar_gz`
- developer environment: `locate_executable`, `show_path`, `git_status`, `git_log_compact`, `git_branch_list`, `git_diff`, `npm_install`, `npm_run`, `python_version`, `rust_version`
- permission/admin-sensitive commands: `change_permission`, `change_owner`

Examples:

| Source command | Target OS | Native translation |
| --- | --- | --- |
| `ipconfig` | macOS | `ifconfig` |
| `dir` | macOS | `ls` |
| `cls` | macOS | `clear` |
| `ls` | Windows | `Get-ChildItem` |
| `clear` | Windows | `Clear-Host` |
| `open .` | Windows | `Invoke-Item .` |
| `head -n 20 app.log` | Windows | `Get-Content app.log -TotalCount 20` |
| `tracert example.com` | macOS | `traceroute example.com` |
| `curl -I https://example.com` | Windows | `Invoke-WebRequest -Method Head https://example.com` |
| `zip -r app.zip app` | Windows | `Compress-Archive -Path app -DestinationPath app.zip` |

## Command Inventory Pipeline

CLI4ALL keeps raw command inventory separate from reviewed runtime mappings.

- `tools/collectors/collect_macos_zsh.sh` collects zsh builtins, aliases, functions, and PATH commands on macOS.
- `tools/collectors/collect_gnu_linux.sh` collects bash builtins, aliases, functions, and PATH commands on GNU/Linux.
- `tools/collectors/collect_powershell.ps1` collects PowerShell commands from `Get-Command *`.
- `tools/collectors/collect_windows_cmd.ps1` collects Windows CMD internal commands from `cmd /c help`, a seed list, and discoverable external applications.
- `tools/collectors/generate_command_candidates.py` reads raw inventories and writes `data/commands.candidates.json`.

Raw inventory strategy:

- `data/raw/generated/` is for machine-generated inventories and is ignored by git.
- `data/raw/samples/` contains small committed sample inventories used to exercise the candidate pipeline.
- Raw inventories are never merged automatically into `data/commands.source.json`.
- Every candidate must be reviewed for intent, argument behavior, and safety before promotion.

Current limitation:

- shell-variable syntax such as `echo %VAR%`, `$env:VAR`, and `echo $VAR` is platform-specific and does not fit the current placeholder model cleanly enough for automatic reviewed mapping yet.

## Official Command Sources

- Windows CMD reference: <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/windows-commands>
- PowerShell command discovery: <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/get-command?view=powershell-7.5>
- GNU Coreutils manual: <https://www.gnu.org/software/coreutils/manual/coreutils.html>
- zsh shell builtins reference: <https://zsh.sourceforge.io/Doc/Release/Shell-Builtin-Commands.html>

## Development

```bash
cargo fmt
cargo test
```

Useful checks:

```bash
cargo run -- check "ipconfig"
cargo run -- translate "dir" --to ubuntu
cargo run -- shell
```

## Desktop App

The desktop app lives in `desktop/`.

Local development flow:

```bash
cd desktop
npm install
npm run tauri dev
```

This opens a Tauri window with an xterm.js terminal surface, a real PTY-backed shell session, and a visible mode switch in the header.

Desktop modes:

- Native Mode behaves like a normal terminal. Raw keyboard input is sent directly to the PTY and the real shell prompt comes from the native shell.
- Translate Mode buffers one local line, translates remembered cross-platform commands through the Rust CLI4ALL logic, applies the existing safety rules, and writes only the translated native command to the PTY.

## Desktop App Preview

`desktop/` is the macOS desktop packaging target for the Tauri app. The UI exposes a visible Native Mode / Translate Mode toggle, and `npm run tauri build` can produce `.app` and `.dmg` artifacts on macOS.

## Roadmap

Phase 1:

- CLI helper commands
- `cli4all shell` prototype
- single-line translated command execution

Next:

- polish the PTY-backed desktop session UX
- expand installer coverage beyond macOS
- add stronger release automation for desktop artifacts

## Packaging

Release artifacts stay offline and deterministic. Installed users can run:

```bash
cli4all shell
```

See [PACKAGING.md](/Users/user/Desktop/fq/CLI4All/PACKAGING.md) for packaging details across Debian, macOS, and Windows.
