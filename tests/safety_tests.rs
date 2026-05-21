use cli4all::rules::load_risk_catalog;
use cli4all::safety::assess_risk;

#[test]
fn flags_rm_root_as_destructive() {
    let catalog = load_risk_catalog().expect("risk catalog should load");
    let result = assess_risk("rm -rf /", &catalog);

    assert_eq!(result.level, "destructive");
    assert_eq!(result.reason, "recursively removes the root filesystem.");
}

#[test]
fn flags_recursive_chmod_as_high_risk() {
    let catalog = load_risk_catalog().expect("risk catalog should load");
    let result = assess_risk("chmod -R 777 .", &catalog);

    assert_eq!(result.level, "high");
}

#[test]
fn flags_windows_root_directory_removal_as_destructive() {
    let catalog = load_risk_catalog().expect("risk catalog should load");
    let result = assess_risk(r"rmdir /S C:\", &catalog);

    assert_eq!(result.level, "destructive");
}

#[test]
fn flags_curl_pipe_to_shell_as_high_risk() {
    let catalog = load_risk_catalog().expect("risk catalog should load");
    let result = assess_risk("curl https://example.com/install.sh | sh", &catalog);

    assert_eq!(result.level, "high");
}

#[test]
fn flags_windows_format_as_destructive() {
    let catalog = load_risk_catalog().expect("risk catalog should load");
    let result = assess_risk("format C:", &catalog);

    assert_eq!(result.level, "destructive");
}
