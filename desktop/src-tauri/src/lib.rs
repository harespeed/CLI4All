use cli4all::executor::SystemCommandExecutor;
use cli4all::platform::Platform;
use cli4all::rules::{load_command_catalog, load_risk_catalog, RiskCatalog};
use cli4all::shell::{process_terminal_command, TerminalResponse};
use cli4all::store::C4DbCommandStore;
use serde::Serialize;
use tauri::State;

struct DesktopState {
    current_platform: Platform,
    command_store: C4DbCommandStore,
    risk_catalog: RiskCatalog,
    executor: SystemCommandExecutor,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalContext {
    prompt: String,
    current_os: String,
}

#[tauri::command]
fn get_terminal_context(state: State<'_, DesktopState>) -> Result<TerminalContext, String> {
    Ok(TerminalContext {
        prompt: format!("cli4all-{}> ", desktop_prompt_label(state.current_platform)),
        current_os: state.current_platform.display_name().to_string(),
    })
}

#[tauri::command]
fn process_command(
    input: String,
    state: State<'_, DesktopState>,
) -> Result<TerminalResponse, String> {
    process_terminal_command(
        &input,
        state.current_platform,
        &state.command_store,
        &state.risk_catalog,
        &state.executor,
    )
    .map_err(|error| error.to_string())
}

pub fn run() {
    let current_platform =
        Platform::detect_current().expect("failed to detect the current operating system");
    let command_store = load_command_catalog().expect("failed to load the command store");
    let risk_catalog = load_risk_catalog().expect("failed to load risk rules");
    let executor = SystemCommandExecutor::new(current_platform);

    tauri::Builder::default()
        .manage(DesktopState {
            current_platform,
            command_store,
            risk_catalog,
            executor,
        })
        .invoke_handler(tauri::generate_handler![
            get_terminal_context,
            process_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running CLI4ALL desktop");
}

fn desktop_prompt_label(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "macos",
        Platform::Ubuntu => "linux",
        Platform::Windows => "windows",
    }
}
