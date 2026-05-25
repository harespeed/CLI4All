mod pty;

use cli4all::platform::Platform;
use cli4all::rules::{load_command_catalog, load_risk_catalog, RiskCatalog};
use cli4all::shell::{decide_shell_command, ShellAction};
use cli4all::store::formats::read_source_catalog;
use cli4all::store::{C4DbCommandStore, CommandRecord};
use cli4all::translator::display_platform_name;
use pty::PtySession;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

const CONFIRMATION_PROMPT: &str = "Execute this command? [y/N]";
const BUILTIN_SOURCE: &str = "CLI4ALL Built-in";

struct DesktopState {
    current_platform: Platform,
    command_store: C4DbCommandStore,
    reviewed_catalog: Vec<CommandRecord>,
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

#[derive(Clone)]
struct RunningTranslateCommand {
    command_id: u64,
    child: Arc<Mutex<Child>>,
    interrupted: Arc<AtomicBool>,
}

struct TranslateState {
    initial_cwd: PathBuf,
    cwd: PathBuf,
    home_dir: Option<PathBuf>,
}

struct RuntimeState {
    next_session_id: u64,
    next_translate_command_id: u64,
    active_session: Option<ActiveSession>,
    pending_confirmation: Option<PendingConfirmation>,
    running_translate_command: Option<RunningTranslateCommand>,
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
    stream_command_id: Option<u64>,
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
    stream_command_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslateOutputEvent {
    command_id: u64,
    stream: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslateExitEvent {
    command_id: u64,
    exit_status: Option<i32>,
    interrupted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InterruptTranslateCommandResponse {
    command_id: Option<u64>,
    interrupted: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CatalogSuggestion {
    command_template: String,
    intent_id: String,
    description: String,
    source_shell: String,
    target_shell: String,
    risk: String,
    preview_translation: Option<String>,
    score: i32,
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
    app: AppHandle,
    input: String,
    state: State<'_, DesktopState>,
) -> Result<SubmitTerminalLineResponse, String> {
    {
        let runtime = lock_runtime(&state)?;
        if runtime.pending_confirmation.is_some() {
            return Err("a confirmation is already pending".to_string());
        }
        if runtime.running_translate_command.is_some() {
            return Err("a translate command is already running".to_string());
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
        stream_command_id: None,
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
                let command_id = start_translate_command(
                    &app,
                    &state,
                    state.current_platform,
                    translated_command,
                    &current_dir,
                )?;
                response.stream_command_id = Some(command_id);
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
    app: AppHandle,
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
                stream_command_id: None,
            })
        } else {
            let command_id = start_translate_command(
                &app,
                &state,
                state.current_platform,
                &pending.translated_command,
                &current_dir,
            )?;
            Ok(ConfirmationResolutionResponse {
                action: ConfirmationResolutionAction::Execute,
                translated_command: Some(pending.translated_command),
                message: "Translated command executed.".to_string(),
                stdout: String::new(),
                stderr: String::new(),
                exit_status: None,
                current_dir: display_path(&current_dir),
                clear_display: false,
                stream_command_id: Some(command_id),
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
            stream_command_id: None,
        })
    }
}

#[tauri::command]
fn interrupt_translate_command(
    state: State<'_, DesktopState>,
) -> Result<InterruptTranslateCommandResponse, String> {
    let running = {
        let runtime = lock_runtime(&state)?;
        runtime.running_translate_command.clone()
    };

    let Some(running) = running else {
        return Ok(InterruptTranslateCommandResponse {
            command_id: None,
            interrupted: false,
        });
    };

    running.interrupted.store(true, Ordering::SeqCst);
    let kill_result = running
        .child
        .lock()
        .map_err(|_| "translate child process state is poisoned".to_string())?
        .kill();

    match kill_result {
        Ok(_) => Ok(InterruptTranslateCommandResponse {
            command_id: Some(running.command_id),
            interrupted: true,
        }),
        Err(error) => Err(format!("failed to interrupt translate command: {error}")),
    }
}

#[tauri::command]
fn search_catalog_suggestions(
    query: String,
    target_shell: Option<String>,
    limit: Option<usize>,
    state: State<'_, DesktopState>,
) -> Result<Vec<CatalogSuggestion>, String> {
    let target_platform = target_shell
        .as_deref()
        .and_then(cli4all::platform::normalize_target_platform)
        .unwrap_or(state.current_platform);
    let limit = limit.unwrap_or(5).clamp(1, 10);

    Ok(search_catalog_suggestions_internal(
        &state.reviewed_catalog,
        &query,
        target_platform,
        limit,
    ))
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
            let reviewed_catalog = load_reviewed_command_catalog()
                .map_err(|error| format!("failed to load the reviewed source catalog: {error:#}"))?;
            let risk_catalog =
                load_risk_catalog().map_err(|error| format!("failed to load risk rules: {error:#}"))?;
            let home_dir = home_dir();
            let initial_cwd = initial_translate_cwd(home_dir.clone());

            app.manage(DesktopState {
                current_platform,
                command_store,
                reviewed_catalog,
                risk_catalog,
                runtime: Mutex::new(RuntimeState {
                    next_session_id: 0,
                    next_translate_command_id: 0,
                    active_session: None,
                    pending_confirmation: None,
                    running_translate_command: None,
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
            resolve_confirmation,
            interrupt_translate_command,
            search_catalog_suggestions
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

fn load_reviewed_command_catalog() -> Result<Vec<CommandRecord>, String> {
    let path = cli4all::data_paths::find_data_file("commands.source.json")
        .map_err(|error| error.to_string())?;
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let catalog = read_source_catalog(&bytes)
        .map_err(|error| format!("failed to parse {}: {error:#}", path.display()))?;
    Ok(catalog.commands)
}

fn search_catalog_suggestions_internal(
    catalog: &[CommandRecord],
    query: &str,
    target_platform: Platform,
    limit: usize,
) -> Vec<CatalogSuggestion> {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return Vec::new();
    }

    let query_tokens = normalized_query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    let mut suggestions = Vec::new();

    for record in catalog {
        if record.risk_level == "destructive" {
            continue;
        }

        let target_preview = record.target_commands(target_platform.key()).first().cloned();

        for (source_shell, commands) in record.iter_commands() {
            for command_template in commands {
                let score = catalog_suggestion_score(
                    command_template,
                    &record.intent,
                    &record.description,
                    source_shell,
                    target_platform,
                    &record.risk_level,
                    &query_tokens,
                    &normalized_query,
                );

                if score <= 0 {
                    continue;
                }

                if record.risk_level == "high"
                    && !is_strong_catalog_match(
                        command_template,
                        &record.intent,
                        &normalized_query,
                    )
                {
                    continue;
                }

                let dedupe_key = format!(
                    "{}::{}",
                    source_shell,
                    command_template.to_ascii_lowercase()
                );
                if !seen.insert(dedupe_key) {
                    continue;
                }

                suggestions.push(CatalogSuggestion {
                    command_template: command_template.clone(),
                    intent_id: record.intent.clone(),
                    description: record.description.clone(),
                    source_shell: source_shell.to_string(),
                    target_shell: target_platform.key().to_string(),
                    risk: record.risk_level.clone(),
                    preview_translation: target_preview.clone(),
                    score,
                });
            }
        }
    }

    suggestions.sort_by_key(|suggestion| {
        (
            Reverse(suggestion.score),
            risk_priority(&suggestion.risk),
            suggestion.command_template.len(),
            suggestion.command_template.clone(),
        )
    });
    suggestions.truncate(limit);
    suggestions
}

fn catalog_suggestion_score(
    command_template: &str,
    intent: &str,
    description: &str,
    source_shell: &str,
    target_platform: Platform,
    risk_level: &str,
    query_tokens: &[&str],
    normalized_query: &str,
) -> i32 {
    let command_lower = command_template.to_ascii_lowercase();
    let intent_lower = intent.to_ascii_lowercase();
    let description_lower = description.to_ascii_lowercase();
    let command_tokens = command_lower
        .split_whitespace()
        .map(normalize_search_token)
        .collect::<Vec<_>>();

    let mut score = 0;
    if command_lower == normalized_query {
        score += 100;
    }
    if command_lower.starts_with(normalized_query) {
        score += 80;
    }

    for token in query_tokens {
        if command_tokens
            .iter()
            .any(|command_token| command_token.starts_with(token))
        {
            score += 50;
        }

        if intent_lower.contains(token) {
            score += 30;
        }

        if description_lower.contains(token) {
            score += 20;
        }
    }

    if source_shell_matches_target(source_shell, target_platform) {
        score += 20;
    }

    score += match risk_level {
        "low" => 5,
        "high" => -20,
        _ => 0,
    };

    score
}

fn is_strong_catalog_match(command_template: &str, intent: &str, normalized_query: &str) -> bool {
    let command_lower = command_template.to_ascii_lowercase();
    let intent_lower = intent.to_ascii_lowercase();

    command_lower.starts_with(normalized_query)
        || command_lower
            .split_whitespace()
            .map(normalize_search_token)
            .any(|token| token.starts_with(normalized_query))
        || intent_lower.contains(normalized_query)
}

fn source_shell_matches_target(source_shell: &str, target_platform: Platform) -> bool {
    match target_platform {
        Platform::Windows => matches!(source_shell, "windows_cmd" | "powershell"),
        Platform::Macos => source_shell == "macos",
        Platform::Ubuntu => source_shell == "ubuntu",
    }
}

fn normalize_search_token(token: &str) -> String {
    token
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_ascii_lowercase()
}

fn risk_priority(risk: &str) -> u8 {
    match risk {
        "low" => 0,
        "medium" => 1,
        "high" => 2,
        "destructive" => 3,
        _ => 4,
    }
}

fn start_translate_command(
    app: &AppHandle,
    state: &State<'_, DesktopState>,
    current_platform: Platform,
    translated_command: &str,
    current_dir: &Path,
) -> Result<u64, String> {
    if translated_command.contains('\n') || translated_command.contains('\r') {
        return Err("Translate Mode only supports single-line commands".to_string());
    }

    let mut command = build_translate_command(current_platform, translated_command, current_dir);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn translated command: {error}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(child));
    let interrupted = Arc::new(AtomicBool::new(false));

    let (command_id, running) = {
        let mut runtime = lock_runtime(state)?;
        if runtime.running_translate_command.is_some() {
            return Err("a translate command is already running".to_string());
        }
        runtime.next_translate_command_id += 1;
        let command_id = runtime.next_translate_command_id;
        let running = RunningTranslateCommand {
            command_id,
            child: Arc::clone(&child),
            interrupted: Arc::clone(&interrupted),
        };
        runtime.running_translate_command = Some(running.clone());
        (command_id, running)
    };

    let app = app.clone();

    if let Some(stdout) = stdout {
        spawn_translate_stream_reader(app.clone(), command_id, "stdout", stdout);
    }

    if let Some(stderr) = stderr {
        spawn_translate_stream_reader(app.clone(), command_id, "stderr", stderr);
    }

    spawn_translate_waiter(app, command_id, running);
    Ok(command_id)
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
            stream_command_id: None,
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
                stream_command_id: None,
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
            stream_command_id: None,
        })),
    }
}

fn build_translate_command(
    current_platform: Platform,
    translated_command: &str,
    current_dir: &Path,
) -> Command {
    let mut command = match current_platform {
        Platform::Macos => {
            let mut command = Command::new("/bin/zsh");
            command.args(["-lc", translated_command]);
            command
        }
        Platform::Ubuntu => {
            let mut command = Command::new("/bin/bash");
            command.args(["-lc", translated_command]);
            command
        }
        Platform::Windows => {
            let mut command = Command::new("powershell.exe");
            command.args(["-NoProfile", "-Command", translated_command]);
            command
        }
    };
    command.current_dir(current_dir);
    command
}

fn spawn_translate_stream_reader<R>(
    app: AppHandle,
    command_id: u64,
    stream: &'static str,
    reader: R,
) where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();

        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let _ = app.emit(
                        "translate-output",
                        TranslateOutputEvent {
                            command_id,
                            stream: stream.to_string(),
                            text: buffer.clone(),
                        },
                    );
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_translate_waiter(
    app: AppHandle,
    command_id: u64,
    running: RunningTranslateCommand,
) {
    thread::spawn(move || {
        let exit_status = running
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.wait().ok())
            .and_then(|status| status.code());
        let interrupted = running.interrupted.load(Ordering::SeqCst);

        if let Some(state) = app.try_state::<DesktopState>() {
            if let Ok(mut runtime) = state.runtime.lock() {
                if runtime
                    .running_translate_command
                    .as_ref()
                    .map(|active| active.command_id == command_id)
                    .unwrap_or(false)
                {
                    runtime.running_translate_command = None;
                }
            }
        }

        let _ = app.emit(
            "translate-exit",
            TranslateExitEvent {
                command_id,
                exit_status,
                interrupted,
            },
        );
    });
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

#[cfg(test)]
mod tests {
    use super::{
        load_reviewed_command_catalog, native_pwd_command, parse_cd_command, resolve_cd_target_with_home,
        search_catalog_suggestions_internal,
    };
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

    #[test]
    fn catalog_search_returns_curl_suggestions() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "cur", Platform::Windows, 5);

        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.command_template.starts_with("curl -I")),
            "curl header suggestion should appear"
        );
    }

    #[test]
    fn catalog_search_returns_head_suggestions() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "head", Platform::Windows, 5);

        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.intent_id == "head_file"),
            "head_file should be discoverable"
        );
    }

    #[test]
    fn catalog_search_matches_port_related_intents() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "port", Platform::Macos, 5);

        assert!(
            suggestions.iter().any(|suggestion| {
                suggestion.intent_id == "process_by_port"
                    || suggestion.intent_id == "list_listening_ports"
                    || suggestion.intent_id == "check_port"
            }),
            "port query should surface reviewed port-related intents"
        );
    }

    #[test]
    fn catalog_search_matches_git_status() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "git s", Platform::Macos, 5);

        assert_eq!(suggestions.first().map(|item| item.intent_id.as_str()), Some("git_status"));
    }

    #[test]
    fn catalog_search_marks_npm_risk() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "npm", Platform::Macos, 5);

        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.intent_id == "npm_install" && suggestion.risk == "medium")
        );
        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.intent_id == "npm_run" && suggestion.risk == "medium")
        );
    }

    #[test]
    fn catalog_search_hides_destructive_suggestions() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "rm -rf /", Platform::Macos, 5);

        assert!(
            suggestions
                .iter()
                .all(|suggestion| suggestion.risk != "destructive"),
            "destructive suggestions should not be returned"
        );
    }

    #[test]
    fn catalog_search_can_surface_high_risk_permission_changes() {
        let catalog = load_reviewed_command_catalog().expect("catalog should load");
        let suggestions = search_catalog_suggestions_internal(&catalog, "chmod", Platform::Macos, 5);

        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.intent_id == "change_permission" && suggestion.risk == "high")
        );
    }
}
