mod pty;

use cli4all::platform::Platform;
use cli4all::rules::{load_command_catalog, load_risk_catalog, RiskCatalog};
use cli4all::shell::{decide_shell_command, ShellAction};
use cli4all::store::C4DbCommandStore;
use cli4all::translator::display_platform_name;
use pty::PtySession;
use serde::Serialize;
use std::env;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager, State};

const CONFIRMATION_PROMPT: &str = "Execute this command? [y/N]";

struct DesktopState {
    current_platform: Platform,
    command_store: C4DbCommandStore,
    risk_catalog: RiskCatalog,
    runtime: Mutex<RuntimeState>,
}

#[derive(Clone)]
struct ActiveSession {
    session_id: u64,
    pty: PtySession,
}

struct PendingConfirmation {
    session_id: u64,
    translated_command: String,
}

struct RuntimeState {
    next_session_id: u64,
    active_session: Option<ActiveSession>,
    pending_confirmation: Option<PendingConfirmation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionStartResponse {
    session_id: u64,
    current_os: String,
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

    let active_session = ActiveSession { session_id, pty };

    let mut runtime = lock_runtime(&state)?;
    runtime.pending_confirmation = None;
    runtime.active_session = Some(active_session);

    Ok(SessionStartResponse {
        session_id,
        current_os: current_platform.display_name().to_string(),
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
    let decision = decide_shell_command(
        &input,
        state.current_platform.key(),
        &state.command_store,
        &state.risk_catalog,
    )
    .map_err(|error| error.to_string())?;

    let session = active_session(&state)?;

    {
        let runtime = lock_runtime(&state)?;
        if runtime.pending_confirmation.is_some() {
            return Err("a confirmation is already pending".to_string());
        }
    }

    let response = SubmitTerminalLineResponse {
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
    };

    match decision.action {
        ShellAction::Execute => {
            let translated_command = decision
                .translated_command
                .as_deref()
                .ok_or_else(|| "missing translated command".to_string())?;
            session
                .pty
                .write_text(&format!("{translated_command}\n"))
                .map_err(|error| error.to_string())?;
        }
        ShellAction::Confirm => {
            let translated_command = decision
                .translated_command
                .ok_or_else(|| "missing translated command".to_string())?;
            let mut runtime = lock_runtime(&state)?;
            runtime.pending_confirmation = Some(PendingConfirmation {
                session_id: session.session_id,
                translated_command,
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
    let (session, pending) = {
        let mut runtime = lock_runtime(&state)?;
        let pending = runtime
            .pending_confirmation
            .take()
            .ok_or_else(|| "no confirmation is pending".to_string())?;
        let session = runtime
            .active_session
            .clone()
            .ok_or_else(|| "no PTY session is running".to_string())?;
        (session, pending)
    };

    if session.session_id != pending.session_id {
        return Err("the PTY session changed before confirmation resolved".to_string());
    }

    if confirmed {
        session
            .pty
            .write_text(&format!("{}\n", pending.translated_command))
            .map_err(|error| error.to_string())?;
        Ok(ConfirmationResolutionResponse {
            action: ConfirmationResolutionAction::Execute,
            translated_command: Some(pending.translated_command),
            message: "Translated command sent to PTY.".to_string(),
        })
    } else {
        Ok(ConfirmationResolutionResponse {
            action: ConfirmationResolutionAction::Cancelled,
            translated_command: Some(pending.translated_command),
            message: "Execution cancelled.".to_string(),
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

            app.manage(DesktopState {
                current_platform,
                command_store,
                risk_catalog,
                runtime: Mutex::new(RuntimeState {
                    next_session_id: 0,
                    active_session: None,
                    pending_confirmation: None,
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
