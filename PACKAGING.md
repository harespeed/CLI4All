# CLI4ALL Packaging

CLI4ALL v0.1 remains a deterministic offline CLI. Packaging distributes the compiled binary, documentation, the C4DB command store files, the source JSON used to rebuild them, and `risks.yaml`.

After installation, users can run:

```bash
cli4all shell
```

## Shared Layout

All package formats should include:

- `README.md`
- `data/commands.source.json`
- `data/commands.c4idx`
- `data/commands.c4dat`
- `data/commands.yaml`
- `data/risks.yaml`
- the platform binary:
  - `cli4all` on Linux and macOS
  - `cli4all.exe` on Windows

The binary loads command mappings from `commands.c4idx` and `commands.c4dat`, and still loads risk rules from `risks.yaml`. During development and in packaged installs, it looks in:

- `CLI4ALL_DATA_DIR` when set
- `./data` from the current working directory during development
- the repo `data/` directory during local development builds
- the Tauri bundled resource directory at `Contents/Resources/data` inside the macOS app bundle
- a sibling `data/` directory next to the executable in standalone release archives
- `~/.local/share/cli4all/data`
- `/usr/local/share/cli4all/data`
- `/opt/homebrew/share/cli4all/data`
- `/usr/share/cli4all/data/` for Debian compatibility

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

Rebuild the command index before packaging if the source catalog changed:

```bash
cargo run -- build-index --input data/commands.source.json --index data/commands.c4idx --data data/commands.c4dat
```

## macOS

macOS-first test flow.

## macOS Desktop App

Build:

```bash
cd desktop
npm install
npm run tauri build
```

Expected artifacts:

- `desktop/src-tauri/target/release/bundle/macos/CLI4ALL.app`
- `desktop/src-tauri/target/release/bundle/dmg/CLI4ALL_*.dmg`

If DMG generation fails, test the `.app` directly. The `.app` path should not be blocked by the DMG step.

Bundled runtime data:

- Tauri copies `../../data` into the app bundle as `Contents/Resources/data`
- the desktop backend looks for runtime files in this order:
  - `CLI4ALL_DATA_DIR`
  - `./data`
  - the repo `data/` directory for local development
  - the Tauri bundled `Resources/data` directory
  - `~/.local/share/cli4all/data`
  - `/usr/local/share/cli4all/data`
  - `/opt/homebrew/share/cli4all/data`
  - `/usr/share/cli4all/data`
- when files are missing, CLI4ALL prints the required filenames, every searched directory, and a suggestion to reinstall or set `CLI4ALL_DATA_DIR`

Testing:

- open `CLI4ALL.app` directly
- verify the desktop header shows the visible Native Mode / Translate Mode switch
- test Native Mode: `pwd`, `ls`, `cd ..`, `echo hello`
- test Translate Mode: `ipconfig`, `dir`, `cls`, `rm -rf /`, `unknown_test_command`

Expected behavior:

- Native Mode should send raw keyboard input directly to the PTY-backed native shell
- Translate Mode should buffer the original line locally and only send the translated native command to PTY
- `ipconfig` should translate to the macOS native command and execute through the PTY
- `dir` should translate to an `ls`-style command
- `cls` should translate to `clear`
- `rm -rf /` should be blocked
- unknown commands should not auto-execute in Translate Mode

Build:

```bash
chmod +x scripts/build_macos_release.sh
./scripts/build_macos_release.sh
```

Extract:

```bash
mkdir -p /tmp/cli4all-test
tar -xzf dist/cli4all-macos-<arch>.tar.gz -C /tmp/cli4all-test
```

Install:

```bash
cd /tmp/cli4all-test/cli4all-macos-<arch>
chmod +x install_macos.sh
./install_macos.sh
```

Test:

```bash
cli4all --help
cli4all check "ipconfig"
cli4all shell
```

GitHub Actions release builds:

- the macOS job builds the current runner host, runs the tests, and packages a tarball in `dist/`
- the workflow names the archive `cli4all-macos-x86_64.tar.gz` on Intel runners or `cli4all-macos-universal.tar.gz` when a non-Intel runner naming fallback is used

The script:

- runs `cargo build --release`
- rebuilds `commands.c4idx` and `commands.c4dat` from `commands.source.json`
- packages the current host build and does not cross-compile by default
- maps the host architecture to `aarch64` or `x86_64`
- creates `dist/cli4all-macos-aarch64.tar.gz` on `aarch64-apple-darwin`
- creates `dist/cli4all-macos-x86_64.tar.gz` on `x86_64-apple-darwin`
- includes `cli4all`, `README.md`, `PACKAGING.md`, `install_macos.sh`, and `data/`

Expected output:

```text
dist/cli4all-macos-aarch64.tar.gz
dist/cli4all-macos-x86_64.tar.gz
```

Installation:

```bash
tar -xzf dist/cli4all-macos-aarch64.tar.gz
cd cli4all-macos-aarch64
chmod +x install_macos.sh
./install_macos.sh
```

Build notes:

- Build the Apple Silicon package on an Apple Silicon Mac for the default host build path.
- Build the Intel package on an Intel Mac, or only use cross-compilation if you have already configured the target and toolchain yourself.

The install script:

- installs `cli4all` to `~/.local/bin`
- installs runtime data files to `~/.local/share/cli4all/data`
- installs `README.md` and `PACKAGING.md` to `~/.local/share/cli4all`
- avoids `sudo` by default
- prints PATH instructions when `~/.local/bin` is not already on PATH

## Optional macOS PKG

PKG packaging is optional and is not the default macOS release path for CLI4ALL v0.1. Build PKG installers on macOS.

Useful tools:

- `pkgbuild`
- `productbuild`

Notes:

- Keep the tar.gz archive plus `install_macos.sh` as the default macOS release path.
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
- copies `commands.source.json`, `commands.c4idx`, `commands.c4dat`, and `risks.yaml` to `%LOCALAPPDATA%\CLI4ALL\data`
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
