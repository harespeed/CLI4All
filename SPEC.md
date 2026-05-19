# CLI4ALL SPEC

## Product Goal

CLI4ALL is a deterministic cross-platform command translation terminal.

Users type the command they remember. CLI4ALL detects the source platform, translates the intent into the correct native command for the current OS, safety-checks it, and prints the real output when execution is allowed.

The product stays offline and does not use LLMs or online APIs.

The long-term product is an independent desktop terminal application, similar in role to iTerm or Windows Terminal, with CLI translation built into the terminal experience.

## Current Platform Mapping

- `macos` host -> `macos`
- `linux` host -> `ubuntu` for Phase 1
- `windows` host -> `windows`

Linux distro detection is not implemented yet. Phase 1 uses the Ubuntu command set for Linux hosts.

## Core Commands

### check

Example:

`cli4all check "ipconfig"`

Expected behavior:

- Detect the remembered command deterministically.
- Explain the source platform and known native equivalent data from the command store.

### translate

Example:

`cli4all translate "dir" --to ubuntu`

Expected behavior:

- Detect that `dir` is a Windows CMD command.
- Return known native equivalents for the requested target platform.

### explain

Example:

`cli4all explain "chmod -R 777 ."`

Expected behavior:

- Explain known tokens deterministically.
- Report the assessed risk.

### risk

Example:

`cli4all risk "rm -rf /"`

Expected behavior:

- Risk level: destructive.
- Reason: recursively removes root filesystem.

### fix

Example:

`cli4all fix "command not found: ipconfig"`

Expected behavior:

- Identify the remembered foreign command.
- Suggest the known native alternative commands.

### shell

Example:

`cli4all shell`

Expected behavior:

- Start an interactive prompt such as `cli4all-macos>`.
- Read one input line at a time.
- Exit on `exit` or `quit`.
- For each input:
  - detect the source platform
  - match the intent from the command store
  - translate it to the current OS
  - assess risk
  - print execution metadata
  - execute only when policy allows it

Printed metadata:

- `Original command: ...`
- `Detected source: ...`
- `Current OS: ...`
- `Matched intent: ...`
- `Translated command: ...`
- `Risk level: low|medium|high|destructive`

Execution policy:

- Low-risk known translated commands execute automatically.
- Medium-risk commands ask: `Execute this medium-risk command? [y/N]`
- High-risk commands ask: `Execute this high-risk command? [y/N]`
- Destructive commands are blocked.
- Unknown commands are not auto-executed in Phase 1.

### desktop

Phase 2 adds a desktop terminal foundation in `desktop/`:

- Tauri window shell
- React + TypeScript frontend
- xterm.js terminal rendering
- Rust backend command processing via Tauri commands
- reuse of the same deterministic translation, safety, and execution logic as `cli4all shell`

Phase 2 desktop behavior:

- show an initial prompt such as `cli4all-macos>` or `cli4all-linux>`
- accept keyboard input in xterm.js
- send the submitted line to the Rust backend on Enter
- return a structured terminal response containing translation metadata, output, exit status, and action
- require confirmation responses for medium and high risk commands
- block destructive commands
- refuse unknown commands in safe mode

## Safety Boundary

CLI4ALL does not blindly execute user input. In interactive shell mode, CLI4ALL executes only known translated native commands after detection, translation, and safety checks.

Rules:

- Foreign commands are never executed directly.
- Execution is allowed only for known translated native commands.
- Unknown commands are not auto-executed in Phase 1.
- Destructive commands are blocked by default.
- CLI4ALL always prints the translated command before execution.

## Non-goals for Phase 2

- No full PTY emulation yet.
- No real shell session state yet.
- No working directory persistence yet.
- No Ctrl+C handling beyond default process behavior.
- No tabs, panes, SSH integration, or full-screen terminal multiplexing.
- No LLM agent.
- No online APIs.

## Tech Stack

- Rust 2021 edition
- clap for CLI parsing
- serde and serde_yaml / serde_json for deterministic data
- regex for command detection and risk rules
- anyhow for error handling
- `std::process::Command` for Phase 1 and Phase 2 execution
- Tauri + React + TypeScript + xterm.js for the Phase 2 desktop foundation

## Data Model

- command mappings live in `data/commands.source.json`
- runtime lookup uses `data/commands.c4idx` and `data/commands.c4dat`
- regex safety rules live in `data/risks.yaml`

## Roadmap

Phase 1:

- CLI helper
- `cli4all shell` prototype
- single-line translated command execution

Phase 2:

- Tauri desktop terminal app foundation
- xterm.js UI
- command translation via Rust backend

Phase 3:

- PTY backend
- real shell sessions
- working directory/session state
- Ctrl+C handling

Phase 4:

- production installers
- `.dmg` and `.app`
- `.msi`
- `.deb` and AppImage
