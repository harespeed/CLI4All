mod pty;

use cli4all::platform::Platform;
use cli4all::rules::{load_command_catalog, load_risk_catalog, RiskCatalog};
use cli4all::shell::{decide_shell_command, ShellAction};
use cli4all::store::C4DbCommandStore;
use cli4all::translator::display_platform_name;
use pty::PtySession;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager, State};

const CONFIRMATION_PROMPT: &str = "Execute this command? [y/N]";
const BUILTIN_SOURCE: &str = "CLI4ALL Built-in";

struct DesktopState {
    current_platform: Platform,
    command_store: C4DbCommandStore,
    risk_catalog: RiskCatalog,
    runtime: Mutex<RuntimeState>,
}

#[derive(Clone)]
struct ActiveSession {
    pty: PtySession,
}

struct PendingConfirmation {
    translated_command: String,
    clear_display: bool,
}

struct TranslateState {
    initial_cwd: PathBuf,
    cwd: PathBuf,
    home_dir: Option<PathBuf>,
}

struct RuntimeState {
    next_session_id: u64,
    active_session: Option<ActiveSession>,
    pending_confirmation: Option<PendingConfirmation>,
    translate: TranslateState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionStartResponse {
    session_id: u64,
    current_os: String,
    current_dir: String,
    home_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitTerminalLineResponse {
    original_command: String,
    detected_source: String,
    current_os: String,
    matched_intent: Option<String>,
    translated_command: Option<String>,
    risk_level: String,
    action: ShellAction,
    risk_reason: Option<String>,
    message: Option<String>,
    confirmation_prompt: Option<String>,
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
    current_dir: String,
    clear_display: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConfirmationResolutionAction {
    Execute,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmationResolutionResponse {
    action: ConfirmationResolutionAction,
    translated_command: Option<String>,
    message: String,
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
    current_dir: String,
    clear_display: bool,
}

#[tauri::command]
fn start_pty_session(
    app: AppHandle,
    cols: u16,
    rows: u16,
    state: State<'_, DesktopState>,
) -> Result<SessionStartResponse, String> {
    let (current_platform, session_id, previous_session) = {
        let mut runtime = lock_runtime(&state)?;
        let previous_session = runtime.active_session.take();
        runtime.pending_confirmation = None;
        runtime.translate.cwd = runtime.translate.initial_cwd.clone();
        runtime.next_session_id += 1;
        (
            state.current_platform,
            runtime.next_session_id,
            previous_session,
        )
    };

    if let Some(session) = previous_session {
        let _ = session.pty.stop();
    }

    let pty = PtySession::start(app, session_id, current_platform, cols, rows)
        .map_err(|error| format!("failed to start PTY session: {error:#}"))?;

    let active_session = ActiveSession { pty };

    let mut runtime = lock_runtime(&state)?;
    runtime.pending_confirmation = None;
    runtime.active_session = Some(active_session);

    Ok(SessionStartResponse {
        session_id,
        current_os: current_platform.display_name().to_string(),
        current_dir: display_path(&runtime.translate.cwd),
        home_dir: runtime
            .translate
            .home_dir
            .as_deref()
            .map(display_path),
    })
}

#[tauri::command]
fn write_to_pty(input: String, state: State<'_, DesktopState>) -> Result<(), String> {
    let session = active_session(&state)?;
    session
        .pty
        .write_text(&input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn resize_pty(cols: u16, rows: u16, state: State<'_, DesktopState>) -> Result<(), String> {
    let session = active_session(&state)?;
    session
        .pty
        .resize(cols, rows)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn stop_pty_session(state: State<'_, DesktopState>) -> Result<(), String> {
    let previous_session = {
        let mut runtime = lock_runtime(&state)?;
        runtime.pending_confirmation = None;
        runtime.active_session.take()
    };

    if let Some(session) = previous_session {
        let _ = session.pty.stop();
    }

    Ok(())
}

#[tauri::command]
fn submit_terminal_line(
    input: String,
    state: State<'_, DesktopState>,
) -> Result<SubmitTerminalLineResponse, String> {
    {
        let runtime = lock_runtime(&state)?;
        if runtime.pending_confirmation.is_some() {
            return Err("a confirmation is already pending".to_string());
        }
    }

    {
        let mut runtime = lock_runtime(&state)?;
        if let Some(response) =
            handle_translate_builtin(&input, state.current_platform, &mut runtime)?
        {
            return Ok(response);
        }
    }

    let decision = decide_shell_command(
        &input,
        state.current_platform.key(),
        &state.command_store,
        &state.risk_catalog,
    )
    .map_err(|error| error.to_string())?;

    let current_dir = {
        let runtime = lock_runtime(&state)?;
        runtime.translate.cwd.clone()
    };

    let mut response = SubmitTerminalLineResponse {
        original_command: decision.original_command.clone(),
        detected_source: display_platform_name(&decision.detected_source).to_string(),
        current_os: state.current_platform.display_name().to_string(),
        matched_intent: decision.intent.clone(),
        translated_command: decision.translated_command.clone(),
        risk_level: decision.risk_level.clone(),
        action: decision.action.clone(),
        risk_reason: decision.risk_reason.clone(),
        message: submit_message(&decision.action),
        confirmation_prompt: matches!(decision.action, ShellAction::Confirm)
            .then(|| CONFIRMATION_PROMPT.to_string()),
        stdout: String::new(),
        stderr: String::new(),
        exit_status: None,
        current_dir: display_path(&current_dir),
        clear_display: false,
    };

    match decision.action {
        ShellAction::Execute => {
            let translated_command = decision
                .translated_command
                .as_deref()
                .ok_or_else(|| "missing translated command".to_string())?;

            if is_clear_screen_intent(decision.intent.as_deref()) {
                response.clear_display = true;
                response.exit_status = Some(0);
            } else {
                let result =
                    execute_translated_command(state.current_platform, translated_command, &current_dir)?;
                response.stdout = result.stdout;
                response.stderr = result.stderr;
                response.exit_status = result.exit_status;
            }
        }
        ShellAction::Confirm => {
            let translated_command = decision
                .translated_command
                .ok_or_else(|| "missing translated command".to_string())?;
            let mut runtime = lock_runtime(&state)?;
            runtime.pending_confirmation = Some(PendingConfirmation {
                translated_command,
                clear_display: is_clear_screen_intent(decision.intent.as_deref()),
            });
        }
        ShellAction::Block | ShellAction::UnknownNoExecute => {}
    }

    Ok(response)
}

#[tauri::command]
fn resolve_confirmation(
    confirmed: bool,
    state: State<'_, DesktopState>,
) -> Result<ConfirmationResolutionResponse, String> {
    let (pending, current_dir) = {
        let mut runtime = lock_runtime(&state)?;
        let pending = runtime
            .pending_confirmation
            .take()
            .ok_or_else(|| "no confirmation is pending".to_string())?;
        (pending, runtime.translate.cwd.clone())
    };

    if confirmed {
        if pending.clear_display {
            Ok(ConfirmationResolutionResponse {
                action: ConfirmationResolutionAction::Execute,
                translated_command: Some(pending.translated_command),
                message: "Translated command executed.".to_string(),
                stdout: String::new(),
                stderr: String::new(),
                exit_status: Some(0),
                current_dir: display_path(&current_dir),
                clear_display: true,
            })
        } else {
            let result = execute_translated_command(
                state.current_platform,
                &pending.translated_command,
                &current_dir,
            )?;
            Ok(ConfirmationResolutionResponse {
                action: ConfirmationResolutionAction::Execute,
                translated_command: Some(pending.translated_command),
                message: "Translated command executed.".to_string(),
                stdout: result.stdout,
                stderr: result.stderr,
                exit_status: result.exit_status,
                current_dir: display_path(&current_dir),
                clear_display: false,
            })
        }
    } else {
        Ok(ConfirmationResolutionResponse {
            action: ConfirmationResolutionAction::Cancelled,
            translated_command: Some(pending.translated_command),
            message: "Execution cancelled.".to_string(),
            stdout: String::new(),
            stderr: String::new(),
            exit_status: None,
            current_dir: display_path(&current_dir),
            clear_display: false,
        })
    }
}

pub fn run() {
    let current_platform =
        Platform::detect_current().expect("failed to detect the current operating system");

    tauri::Builder::default()
        .setup(move |app| {
            if let Ok(resource_dir) = app.path().resource_dir() {
                let bundled_data_dir = resource_dir.join("data");
                if bundled_data_dir.is_dir() {
                    env::set_var(
                        cli4all::data_paths::BUNDLED_DATA_DIR_ENV_VAR,
                        bundled_data_dir,
                    );
                }
            }

            let command_store = load_command_catalog()
                .map_err(|error| format!("failed to load the command store: {error:#}"))?;
            let risk_catalog =
                load_risk_catalog().map_err(|error| format!("failed to load risk rules: {error:#}"))?;
            let home_dir = home_dir();
            let initial_cwd = initial_translate_cwd(home_dir.clone());

            app.manage(DesktopState {
                current_platform,
                command_store,
                risk_catalog,
                runtime: Mutex::new(RuntimeState {
                    next_session_id: 0,
                    active_session: None,
                    pending_confirmation: None,
                    translate: TranslateState {
                        initial_cwd: initial_cwd.clone(),
                        cwd: initial_cwd,
                        home_dir,
                    },
                }),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_pty_session,
            write_to_pty,
            resize_pty,
            stop_pty_session,
            submit_terminal_line,
            resolve_confirmation
        ])
        .run(tauri::generate_context!())
        .expect("error while running CLI4ALL desktop");
}

fn lock_runtime<'a>(
    state: &'a State<'_, DesktopState>,
) -> Result<MutexGuard<'a, RuntimeState>, String> {
    state
        .runtime
        .lock()
        .map_err(|_| "desktop runtime state is poisoned".to_string())
}

fn active_session(state: &State<'_, DesktopState>) -> Result<ActiveSession, String> {
    let mut runtime = lock_runtime(state)?;

    match runtime.active_session.clone() {
        Some(session) if session.pty.is_alive() => Ok(session),
        Some(_) => {
            runtime.pending_confirmation = None;
            runtime.active_session = None;
            Err("PTY session is not running".to_string())
        }
        None => Err("no PTY session is running".to_string()),
    }
}

fn submit_message(action: &ShellAction) -> Option<String> {
    match action {
        ShellAction::Execute => None,
        ShellAction::Confirm => Some("Confirmation required before execution.".to_string()),
        ShellAction::Block => Some("Destructive command blocked by CLI4ALL.".to_string()),
        ShellAction::UnknownNoExecute => Some(
            "Unknown command mapping. CLI4ALL will not execute this command automatically in safe mode."
                .to_string(),
        ),
    }
}

fn handle_translate_builtin(
    input: &str,
    current_platform: Platform,
    runtime: &mut RuntimeState,
) -> Result<Option<SubmitTerminalLineResponse>, String> {
    let trimmed = input.trim();

    if trimmed.eq_ignore_ascii_case("pwd") {
        let current_dir = display_path(&runtime.translate.cwd);
        return Ok(Some(SubmitTerminalLineResponse {
            original_command: trimmed.to_string(),
            detected_source: BUILTIN_SOURCE.to_string(),
            current_os: current_platform.display_name().to_string(),
            matched_intent: Some("print_working_directory".to_string()),
            translated_command: Some(native_pwd_command(current_platform).to_string()),
            risk_level: "low".to_string(),
            action: ShellAction::Execute,
            risk_reason: None,
            message: None,
            confirmation_prompt: None,
            stdout: format!("{current_dir}\n"),
            stderr: String::new(),
            exit_status: Some(0),
            current_dir,
            clear_display: false,
        }));
    }

    let Some(target) = parse_cd_command(trimmed) else {
        return Ok(None);
    };

    let translated_command = Some(if target.is_empty() {
        "cd".to_string()
    } else {
        format!("cd {target}")
    });

    let current_dir_before = runtime.translate.cwd.clone();
    match resolve_cd_target(target, &current_dir_before, &runtime.translate.initial_cwd) {
        Ok(next_dir) => {
            runtime.translate.cwd = next_dir.clone();
            Ok(Some(SubmitTerminalLineResponse {
                original_command: trimmed.to_string(),
                detected_source: BUILTIN_SOURCE.to_string(),
                current_os: current_platform.display_name().to_string(),
                matched_intent: Some("change_directory".to_string()),
                translated_command,
                risk_level: "low".to_string(),
                action: ShellAction::Execute,
                risk_reason: None,
                message: None,
                confirmation_prompt: None,
                stdout: String::new(),
                stderr: String::new(),
                exit_status: Some(0),
                current_dir: display_path(&runtime.translate.cwd),
                clear_display: false,
            }))
        }
        Err(error) => Ok(Some(SubmitTerminalLineResponse {
            original_command: trimmed.to_string(),
            detected_source: BUILTIN_SOURCE.to_string(),
            current_os: current_platform.display_name().to_string(),
            matched_intent: Some("change_directory".to_string()),
            translated_command,
            risk_level: "low".to_string(),
            action: ShellAction::Execute,
            risk_reason: None,
            message: None,
            confirmation_prompt: None,
            stdout: String::new(),
            stderr: format!("{error}\n"),
            exit_status: Some(1),
            current_dir: display_path(&current_dir_before),
            clear_display: false,
        })),
    }
}

fn execute_translated_command(
    current_platform: Platform,
    translated_command: &str,
    current_dir: &Path,
) -> Result<TranslateExecutionResult, String> {
    if translated_command.contains('\n') || translated_command.contains('\r') {
        return Err("Translate Mode only supports single-line commands".to_string());
    }

    let output = match current_platform {
        Platform::Macos => Command::new("/bin/zsh")
            .args(["-lc", translated_command])
            .current_dir(current_dir)
            .output()
            .map_err(|error| format!("failed to execute translated command with /bin/zsh: {error}"))?,
        Platform::Ubuntu => Command::new("/bin/bash")
            .args(["-lc", translated_command])
            .current_dir(current_dir)
            .output()
            .map_err(|error| format!("failed to execute translated command with /bin/bash: {error}"))?,
        Platform::Windows => execute_windows_command(translated_command, current_dir)?,
    };

    Ok(TranslateExecutionResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_status: output.status.code(),
    })
}

fn execute_windows_command(
    translated_command: &str,
    current_dir: &Path,
) -> Result<std::process::Output, String> {
    match Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", translated_command])
        .current_dir(current_dir)
        .output()
    {
        Ok(output) => Ok(output),
        Err(primary_error) => Command::new("cmd.exe")
            .args(["/C", translated_command])
            .current_dir(current_dir)
            .output()
            .map_err(|fallback_error| {
                format!(
                    "failed to execute translated command with PowerShell ({primary_error}) or cmd.exe ({fallback_error})"
                )
            }),
    }
}

fn initial_translate_cwd(home_dir: Option<PathBuf>) -> PathBuf {
    env::current_dir()
        .ok()
        .or(home_dir)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn resolve_cd_target(target: &str, current_dir: &Path, initial_dir: &Path) -> Result<PathBuf, String> {
    resolve_cd_target_with_home(target, current_dir, initial_dir, home_dir().as_deref())
}

fn resolve_cd_target_with_home(
    target: &str,
    current_dir: &Path,
    initial_dir: &Path,
    home_dir_override: Option<&Path>,
) -> Result<PathBuf, String> {
    let target = strip_wrapping_quotes(target.trim());
    let candidate = if target.is_empty() || target == "~" {
        home_dir_override
            .map(Path::to_path_buf)
            .unwrap_or_else(|| initial_dir.to_path_buf())
    } else if let Some(home_relative) = target
        .strip_prefix("~/")
        .or_else(|| target.strip_prefix("~\\"))
    {
        home_dir_override
            .map(Path::to_path_buf)
            .unwrap_or_else(|| initial_dir.to_path_buf())
            .join(home_relative)
    } else {
        let raw_path = PathBuf::from(target);
        if raw_path.is_absolute() {
            raw_path
        } else {
            current_dir.join(raw_path)
        }
    };

    let resolved = candidate
        .canonicalize()
        .map_err(|error| format!("cd: {}: {error}", candidate.display()))?;

    if resolved.is_dir() {
        Ok(resolved)
    } else {
        Err(format!("cd: {}: not a directory", resolved.display()))
    }
}

fn parse_cd_command(input: &str) -> Option<&str> {
    if input.eq_ignore_ascii_case("cd") {
        return Some("");
    }

    let rest = input.get(2..)?;
    if !input[..2].eq_ignore_ascii_case("cd") {
        return None;
    }

    rest.chars()
        .next()
        .filter(|character| character.is_whitespace())
        .map(|_| rest.trim())
}

fn strip_wrapping_quotes(input: &str) -> &str {
    if input.len() >= 2 {
        let bytes = input.as_bytes();
        let first = bytes[0];
        let last = bytes[input.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &input[1..input.len() - 1];
        }
    }
    input
}

fn native_pwd_command(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "Get-Location",
        Platform::Macos | Platform::Ubuntu => "pwd",
    }
}

fn is_clear_screen_intent(intent: Option<&str>) -> bool {
    matches!(intent, Some("clear_screen"))
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("USERPROFILE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
}

struct TranslateExecutionResult {
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::{native_pwd_command, parse_cd_command, resolve_cd_target_with_home};
    use cli4all::platform::Platform;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cli4all-desktop-{label}-{}-{unique}-{counter}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn parse_cd_command_supports_parent_navigation() {
        assert_eq!(parse_cd_command("cd .."), Some(".."));
    }

    #[test]
    fn resolve_cd_target_supports_home_directory() {
        let temp = TempDir::new("translate-home");
        let home = temp.path().join("home");
        fs::create_dir_all(&home).expect("home dir should exist");

        let resolved = resolve_cd_target_with_home("~", temp.path(), temp.path(), Some(&home))
            .expect("home resolution should work");

        assert_eq!(
            resolved,
            home.canonicalize().expect("home dir should canonicalize")
        );
    }

    #[test]
    fn resolve_cd_target_supports_parent_directory() {
        let temp = TempDir::new("translate-parent");
        let parent = temp.path().join("root");
        let child = parent.join("child");
        fs::create_dir_all(&child).expect("child dir should exist");

        let resolved =
            resolve_cd_target_with_home("..", &child, &parent, Some(&parent)).expect("parent resolution should work");

        assert_eq!(
            resolved,
            parent
                .canonicalize()
                .expect("parent dir should canonicalize")
        );
    }

    #[test]
    fn native_pwd_command_matches_platform() {
        assert_eq!(native_pwd_command(Platform::Macos), "pwd");
        assert_eq!(native_pwd_command(Platform::Ubuntu), "pwd");
        assert_eq!(native_pwd_command(Platform::Windows), "Get-Location");
    }
}
