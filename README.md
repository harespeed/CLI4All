# CLI4ALL v0.1

CLI4ALL is a deterministic Rust CLI that helps Ubuntu users translate familiar commands from other platforms into Ubuntu-friendly equivalents.

## Constraints

- Rust 2021 edition
- No LLMs
- No online APIs
- No command execution
- YAML-backed command and risk rules

## Commands

```bash
cargo run -- check "ipconfig"
cargo run -- translate "dir" --to ubuntu
cargo run -- explain "chmod -R 777 ."
cargo run -- risk "rm -rf /"
cargo run -- fix "command not found: ipconfig"
```

## Rule Files

- `data/commands.yaml` stores cross-platform command mappings.
- `data/risks.yaml` stores regex-driven safety rules.

## Development

```bash
cargo fmt
cargo test
```

## Debian Packaging

Install the packaging tool:

```bash
cargo install cargo-deb
```

Build the release binary:

```bash
cargo build --release
```

Create the Debian package:

```bash
cargo deb
```

Expected package path:

```text
target/debian/cli4all_0.1.0_amd64.deb
```

The package installs:

- `cli4all` to `/usr/bin/cli4all`
- `README.md` to `/usr/share/doc/cli4all/README.md`
- YAML rule files to `/usr/share/cli4all/data/`

## Cross-Platform Packaging

CLI4ALL packaging stays offline and deterministic. Release artifacts should include the binary, `README.md`, and the YAML rule files in `data/`.

### Ubuntu and Debian

Use `cargo-deb`:

```bash
cargo install cargo-deb
cargo build --release
cargo deb
```

Expected output:

```text
target/debian/cli4all_0.1.0_amd64.deb
```

### macOS

Build a tarball that contains:

- `cli4all`
- `README.md`
- `PACKAGING.md`
- `data/commands.yaml`
- `data/risks.yaml`
- `scripts/install_macos.sh`

Build the archive:

```bash
./scripts/build_macos_release.sh
```

Expected output:

```text
dist/cli4all-macos-aarch64.tar.gz
dist/cli4all-macos-x86_64.tar.gz
```

Install from the extracted archive:

```bash
tar -xzf dist/cli4all-macos-aarch64.tar.gz
cd cli4all-macos-aarch64
chmod +x scripts/install_macos.sh
./scripts/install_macos.sh
```

If the target directory is not writable, the installer prints a clear message telling you to rerun it with `sudo`.

The script packages the current host build and does not cross-compile by default. Build the Apple Silicon archive on an Apple Silicon Mac. Build the Intel archive on an Intel Mac, or only use cross-compilation if you have already configured the target yourself.

The installer prefers `/opt/homebrew/bin` when that directory exists and is writable. Otherwise it installs to `/usr/local/bin`. YAML data is installed under the matching `share/cli4all/data` directory.

### Windows

Build a zip archive that contains:

- `cli4all.exe`
- `README.md`
- `PACKAGING.md`
- `data/commands.yaml`
- `data/risks.yaml`
- `scripts/install_windows.ps1`

Build the archive:

```powershell
.\scripts\build_windows_release.ps1
```

Expected output:

```text
dist\cli4all-windows-x86_64.zip
```

Install from the extracted archive:

```powershell
Expand-Archive dist\cli4all-windows-x86_64.zip -DestinationPath .
cd .\cli4all-windows-x86_64
.\scripts\install_windows.ps1
```

The installer copies `cli4all.exe` into `%LOCALAPPDATA%\CLI4ALL\bin`, places YAML data under `%LOCALAPPDATA%\CLI4ALL\data`, updates the user PATH when needed, and prints a reminder to restart the terminal.

See [PACKAGING.md](/Users/user/Desktop/fq/CLI4All/PACKAGING.md) for the full packaging guide.
