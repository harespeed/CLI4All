use cli4all::rules::{load_command_catalog, load_risk_catalog};
use cli4all::shell::{decide_shell_command, ShellAction};

fn shell_decision(input: &str, target: &str) -> cli4all::shell::ShellDecision {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");
    decide_shell_command(input, target, &store, &risk_catalog)
        .expect("shell decision should succeed")
}

#[test]
fn translates_ipconfig_to_macos_without_executing_foreign_command() {
    let decision = shell_decision("ipconfig", "macos");

    assert_eq!(decision.intent.as_deref(), Some("show_ip_config"));
    assert_eq!(decision.translated_command.as_deref(), Some("ifconfig"));
    assert_ne!(
        decision.translated_command.as_deref(),
        Some(decision.original_command.as_str())
    );
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn translates_dir_to_macos_command() {
    let decision = shell_decision("dir", "macos");

    assert_eq!(decision.intent.as_deref(), Some("list_files"));
    assert_eq!(decision.translated_command.as_deref(), Some("ls"));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn preserves_arguments_when_translating_dir_with_path() {
    let decision = shell_decision("dir Desktop", "macos");

    assert_eq!(decision.intent.as_deref(), Some("list_files"));
    assert_eq!(decision.translated_command.as_deref(), Some("ls Desktop"));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn maps_unix_listing_to_powershell_on_windows() {
    let decision = shell_decision("ls -la", "windows");

    assert_eq!(decision.intent.as_deref(), Some("list_all_files"));
    assert_eq!(
        decision.translated_command.as_deref(),
        Some("Get-ChildItem -Force")
    );
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn blocks_destructive_command() {
    let decision = shell_decision("rm -rf /", "ubuntu");

    assert_eq!(decision.risk_level, "destructive");
    assert_eq!(decision.action, ShellAction::Block);
}

#[test]
fn unknown_command_is_not_auto_executed() {
    let decision = shell_decision("abracadabra", "ubuntu");

    assert_eq!(decision.intent, None);
    assert_eq!(decision.translated_command, None);
    assert_eq!(decision.action, ShellAction::UnknownNoExecute);
}

#[test]
fn low_risk_known_command_is_marked_executable() {
    let decision = shell_decision("open .", "ubuntu");

    assert_eq!(decision.risk_level, "low");
    assert_eq!(decision.translated_command.as_deref(), Some("xdg-open ."));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn translates_pwd_to_windows_without_marking_unknown() {
    let decision = shell_decision("pwd", "windows");

    assert_eq!(decision.intent.as_deref(), Some("print_working_directory"));
    assert_eq!(decision.translated_command.as_deref(), Some("Get-Location"));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn translates_whoami_to_macos_without_marking_unknown() {
    let decision = shell_decision("whoami", "macos");

    assert_eq!(decision.intent.as_deref(), Some("print_current_user"));
    assert_eq!(decision.translated_command.as_deref(), Some("whoami"));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn deleting_a_file_requires_confirmation() {
    let decision = shell_decision("del important.txt", "macos");

    assert_eq!(decision.intent.as_deref(), Some("remove_file"));
    assert_eq!(
        decision.translated_command.as_deref(),
        Some("rm important.txt")
    );
    assert_eq!(decision.action, ShellAction::Confirm);
}

#[test]
fn taskkill_requires_confirmation() {
    let decision = shell_decision("taskkill /PID 123 /F", "macos");

    assert_eq!(decision.intent.as_deref(), Some("kill_process"));
    assert_eq!(decision.translated_command.as_deref(), Some("kill 123"));
    assert_eq!(decision.action, ShellAction::Confirm);
}

#[test]
fn package_install_requires_confirmation() {
    let decision = shell_decision("winget install node", "macos");

    assert_eq!(decision.intent.as_deref(), Some("package_manager_install"));
    assert_eq!(
        decision.translated_command.as_deref(),
        Some("brew install node")
    );
    assert_eq!(decision.action, ShellAction::Confirm);
}

#[test]
fn apt_install_requires_confirmation() {
    let decision = shell_decision("sudo apt install ripgrep", "ubuntu");

    assert_eq!(decision.intent.as_deref(), Some("package_manager_install"));
    assert_eq!(
        decision.translated_command.as_deref(),
        Some("sudo apt install ripgrep")
    );
    assert_eq!(decision.action, ShellAction::Confirm);
}

#[test]
fn curl_piped_to_shell_is_not_auto_executed() {
    let decision = shell_decision("curl https://example.com/install.sh | sh", "macos");

    assert_eq!(decision.risk_level, "high");
    assert_ne!(decision.action, ShellAction::Execute);
}

#[test]
fn format_drive_is_blocked() {
    let decision = shell_decision("format C:", "windows");

    assert_eq!(decision.risk_level, "destructive");
    assert_eq!(decision.action, ShellAction::Block);
}
