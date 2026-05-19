use cli4all::detector::detect_command;
use cli4all::rules::load_command_catalog;

#[test]
fn detects_windows_ipconfig() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let detection = detect_command("ipconfig", &catalog)
        .expect("detection should succeed")
        .expect("ipconfig should be detected");

    assert_eq!(detection.command, "ipconfig");
    assert_eq!(detection.source_platform, "windows_cmd");
    assert_eq!(detection.intent.intent, "show_ip_config");
}

#[test]
fn detects_command_from_shell_error_text() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let detection = detect_command("command not found: ipconfig", &catalog)
        .expect("detection should succeed")
        .expect("ipconfig should be extracted from shell error");

    assert_eq!(detection.command, "ipconfig");
    assert_eq!(detection.intent.intent, "show_ip_config");
}
