# CLI4ALL Packaging

CLI4ALL v0.1 remains a deterministic offline CLI. Packaging only distributes the compiled binary, documentation, and YAML rule files.

## Shared Layout

All package formats should include:

- `README.md`
- `data/commands.yaml`
- `data/risks.yaml`
- the platform binary:
  - `cli4all` on Linux and macOS
  - `cli4all.exe` on Windows

The binary can load YAML rules from:

- the source tree during development
- a sibling `data/` directory in release archives
- a nearby shared install directory used by the install scripts
- `/usr/share/cli4all/data/` for Debian packages

## GitHub Actions Release Builds

Automated release builds live in `.github/workflows/release.yml`.

Triggers:

- manual runs through `workflow_dispatch`
- pushed tags matching `v*`

Behavior:

- each platform job runs `cargo fmt --check` and `cargo test`
- each platform job uploads its artifact with `actions/upload-artifact`
- tag builds also create or update a GitHub Release and attach the generated artifacts

Artifact locations:

- macOS archive: `dist/cli4all-macos-x86_64.tar.gz` or `dist/cli4all-macos-universal.tar.gz`
- Debian package: `target/debian/*.deb`
- Windows archive: `dist/cli4all-windows-x86_64.zip`

## Ubuntu and Debian

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

GitHub Actions release builds:

- the Ubuntu job installs `cargo-deb`, runs the tests, builds the release binary, and publishes `target/debian/*.deb`

Expected output:

```text
target/debian/cli4all_0.1.0_amd64.deb
```

Installed paths:

- binary: `/usr/bin/cli4all`
- docs: `/usr/share/doc/cli4all/README.md`
- data: `/usr/share/cli4all/data/`

## macOS

Build the archive:

```bash
./scripts/build_macos_release.sh
```

GitHub Actions release builds:

- the macOS job builds the current runner host, runs the tests, and packages a tarball in `dist/`
- the workflow names the archive `cli4all-macos-x86_64.tar.gz` on Intel runners or `cli4all-macos-universal.tar.gz` when a non-Intel runner naming fallback is used

The script:

- runs `cargo build --release`
- packages the current host build and does not cross-compile by default
- maps the host architecture to `aarch64` or `x86_64`
- creates `dist/cli4all-macos-aarch64.tar.gz` on `aarch64-apple-darwin`
- creates `dist/cli4all-macos-x86_64.tar.gz` on `x86_64-apple-darwin`
- includes `cli4all`, `README.md`, `PACKAGING.md`, `data/`, and `scripts/install_macos.sh`

Expected output:

```text
dist/cli4all-macos-aarch64.tar.gz
dist/cli4all-macos-x86_64.tar.gz
```

Installation:

```bash
tar -xzf dist/cli4all-macos-aarch64.tar.gz
cd cli4all-macos-aarch64
chmod +x scripts/install_macos.sh
./scripts/install_macos.sh
```

Build notes:

- Build the Apple Silicon package on an Apple Silicon Mac for the default host build path.
- Build the Intel package on an Intel Mac, or only use cross-compilation if you have already configured the target and toolchain yourself.

The install script:

- prefers `/opt/homebrew/bin` when it exists and is writable
- otherwise uses `/usr/local/bin`
- installs YAML data to the matching `share/cli4all/data` directory
- prints a clear `sudo` instruction instead of attempting privilege escalation when permission is missing

## Optional macOS PKG

PKG packaging is optional and is not the default macOS release path for CLI4ALL v0.1. Build PKG installers on macOS.

Useful tools:

- `pkgbuild`
- `productbuild`

Notes:

- Keep the tar.gz archive plus `scripts/install_macos.sh` as the default macOS release path.
- Code signing and notarization require an Apple Developer account for a smoother user experience, especially when distributing outside local development workflows.

## Windows

Build the archive:

```powershell
.\scripts\build_windows_release.ps1
```

GitHub Actions release builds:

- the Windows job runs the tests, builds `target\release\cli4all.exe`, stages the release bundle, and publishes `dist\cli4all-windows-x86_64.zip`

The script:

- runs `cargo build --release`
- copies `target\release\cli4all.exe` into a temporary release folder
- creates `dist\cli4all-windows-x86_64.zip`
- includes `cli4all.exe`, `README.md`, `PACKAGING.md`, `data\`, and `scripts\install_windows.ps1`

Expected output:

```text
dist\cli4all-windows-x86_64.zip
```

Installation:

```powershell
Expand-Archive dist\cli4all-windows-x86_64.zip -DestinationPath .
cd .\cli4all-windows-x86_64
.\scripts\install_windows.ps1
```

The install script:

- copies `cli4all.exe` to `%LOCALAPPDATA%\CLI4ALL\bin`
- copies YAML data to `%LOCALAPPDATA%\CLI4ALL\data`
- creates `%LOCALAPPDATA%\CLI4ALL\bin` if it does not already exist
- adds the user-local bin directory to the user PATH when needed
- does not require administrator privileges
- prints a reminder to restart PowerShell or open a new terminal after installation

## Optional Windows MSI with cargo-wix

MSI packaging is optional and is not the default Windows release path for CLI4ALL v0.1. Build MSI installers on Windows.

Required tools:

- Rust stable toolchain
- `cargo-wix`
- WiX Toolset

Example commands:

```powershell
cargo install cargo-wix
cargo wix init
cargo wix
```

Use this only if you want an MSI-based installer workflow. The default Windows packaging path remains the zip-based release archive built by `.\scripts\build_windows_release.ps1`.
