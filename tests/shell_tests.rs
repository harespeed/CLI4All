use cli4all::rules::{load_command_catalog, load_risk_catalog};
use cli4all::shell::{decide_shell_command, ShellAction};

#[test]
fn translates_ipconfig_to_macos_without_executing_foreign_command() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("ipconfig", "macos", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.intent.as_deref(), Some("show_ip_config"));
    assert_eq!(decision.translated_command.as_deref(), Some("ifconfig"));
    assert_ne!(
        decision.translated_command.as_deref(),
        Some(decision.original_command.as_str())
    );
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn translates_dir_to_ubuntu_command() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("dir", "ubuntu", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.intent.as_deref(), Some("list_files"));
    assert_eq!(decision.translated_command.as_deref(), Some("ls"));
}

#[test]
fn maps_unix_listing_to_powershell_on_windows() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("ls -la", "windows", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.intent.as_deref(), Some("list_files"));
    assert_eq!(
        decision.translated_command.as_deref(),
        Some("Get-ChildItem -Force")
    );
}

#[test]
fn blocks_destructive_command() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("rm -rf /", "ubuntu", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.risk_level, "destructive");
    assert_eq!(decision.action, ShellAction::Block);
}

#[test]
fn unknown_command_is_not_auto_executed() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("abracadabra", "ubuntu", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.intent, None);
    assert_eq!(decision.translated_command, None);
    assert_eq!(decision.action, ShellAction::UnknownNoExecute);
}

#[test]
fn low_risk_known_command_is_marked_executable() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("open .", "ubuntu", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.risk_level, "low");
    assert_eq!(decision.translated_command.as_deref(), Some("xdg-open ."));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn translates_pwd_to_windows_without_marking_unknown() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("pwd", "windows", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.intent.as_deref(), Some("print_working_directory"));
    assert_eq!(decision.translated_command.as_deref(), Some("Get-Location"));
    assert_eq!(decision.action, ShellAction::Execute);
}

#[test]
fn translates_whoami_to_macos_without_marking_unknown() {
    let store = load_command_catalog().expect("command catalog should load");
    let risk_catalog = load_risk_catalog().expect("risk catalog should load");

    let decision = decide_shell_command("whoami", "macos", &store, &risk_catalog)
        .expect("shell decision should succeed");

    assert_eq!(decision.intent.as_deref(), Some("print_current_user"));
    assert_eq!(decision.translated_command.as_deref(), Some("whoami"));
    assert_eq!(decision.action, ShellAction::Execute);
}
