# CLI4ALL v0.1

CLI4ALL lets you type the command you remember and runs the correct version for your current OS.

It remains deterministic and offline. There are no LLMs, no online APIs, and no blind shell passthrough.

## Product Direction

CLI4ALL is becoming an independent desktop terminal application. The current repository includes:

- a Rust CLI core
- `cli4all shell` as the Phase 1 terminal prototype
- a Phase 2 desktop foundation in `desktop/` using Tauri, React, TypeScript, and xterm.js

In shell mode and in the desktop backend, CLI4ALL:

- detects the command source platform
- matches a known intent from the command store
- translates that intent to a native command for the current OS
- runs only the translated native command after safety checks
- prints the real stdout, stderr, and exit status

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
Matched intent: list_files
Translated command: Get-ChildItem -Force
Risk level: low
Running: Get-ChildItem -Force
```

## Rule Files

- `data/commands.source.json` is the source catalog used to build the read-only runtime store.
- `data/commands.c4idx` stores the B+ Tree index used at runtime.
- `data/commands.c4dat` stores serialized command records used at runtime.
- `data/risks.yaml` stores regex-driven safety rules.

Rebuild the runtime store after editing the source catalog:

```bash
cargo run -- build-index --input data/commands.source.json --index data/commands.c4idx --data data/commands.c4dat
```

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

The desktop foundation lives in `desktop/`.

Local development flow:

```bash
cd desktop
npm install
npm run tauri dev
```

This opens a Tauri window with an xterm.js terminal surface. Press Enter to send the current line to the Rust backend. The backend reuses the same translation, safety, and execution rules as `cli4all shell`.

## Roadmap

Phase 1:

- CLI helper commands
- `cli4all shell` prototype
- single-line translated command execution

Phase 2:

- Tauri desktop terminal app foundation
- xterm.js UI
- command translation via Rust backend

Phase 3:

- PTY backend
- real shell sessions
- working directory and session state
- Ctrl+C handling

Phase 4:

- production installers
- `.dmg` and `.app`
- `.msi`
- `.deb` and AppImage

## Packaging

Release artifacts stay offline and deterministic. Installed users can run:

```bash
cli4all shell
```

See [PACKAGING.md](/Users/user/Desktop/fq/CLI4All/PACKAGING.md) for packaging details across Debian, macOS, and Windows.
