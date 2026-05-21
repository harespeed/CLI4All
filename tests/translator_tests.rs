use cli4all::rules::load_command_catalog;
use cli4all::translator::translate_command;

fn suggestions_for(input: &str, target: &str) -> Vec<String> {
    let catalog = load_command_catalog().expect("command catalog should load");
    translate_command(input, target, &catalog)
        .expect("translation should succeed")
        .expect("command should be known")
        .suggestions
}

fn first_suggestion_for(input: &str, target: &str) -> String {
    suggestions_for(input, target)
        .into_iter()
        .next()
        .expect("translation should provide at least one suggestion")
}

#[test]
fn translates_windows_dir_to_macos_ls() {
    assert_eq!(suggestions_for("dir", "macos"), vec!["ls"]);
}

#[test]
fn translates_windows_dir_with_path_to_macos_ls_path() {
    assert_eq!(
        suggestions_for("dir Desktop", "macos"),
        vec!["ls Desktop", "ls"]
    );
}

#[test]
fn translates_windows_cls_to_macos_clear() {
    assert_eq!(suggestions_for("cls", "macos"), vec!["clear"]);
}

#[test]
fn translates_windows_ipconfig_to_macos_ifconfig() {
    assert_eq!(suggestions_for("ipconfig", "macos"), vec!["ifconfig"]);
}

#[test]
fn translates_windows_whoami_to_macos_whoami() {
    assert_eq!(suggestions_for("whoami", "macos"), vec!["whoami"]);
}

#[test]
fn translates_windows_type_file_to_macos_cat_file() {
    assert_eq!(
        suggestions_for("type file.txt", "macos"),
        vec!["cat file.txt"]
    );
}

#[test]
fn translates_findstr_to_grep_on_macos() {
    assert_eq!(
        suggestions_for("findstr error log.txt", "macos"),
        vec!["grep error log.txt"]
    );
}

#[test]
fn translates_where_to_which_on_macos() {
    assert_eq!(suggestions_for("where git", "macos"), vec!["which git"]);
}

#[test]
fn translates_tasklist_to_macos_process_listing() {
    assert_eq!(suggestions_for("tasklist", "macos"), vec!["ps aux"]);
}

#[test]
fn translates_route_print_to_macos_route_table() {
    assert_eq!(suggestions_for("route print", "macos"), vec!["netstat -rn"]);
}

#[test]
fn translates_netstat_to_macos_connection_listing() {
    assert_eq!(
        suggestions_for("netstat -ano", "macos"),
        vec!["netstat -an", "lsof -i"]
    );
}

#[test]
fn translates_unix_ls_to_windows_powershell() {
    assert_eq!(first_suggestion_for("ls", "windows"), "Get-ChildItem");
}

#[test]
fn translates_unix_ls_with_flags_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for("ls -la", "windows"),
        "Get-ChildItem -Force"
    );
}

#[test]
fn translates_unix_clear_to_windows_powershell() {
    assert_eq!(first_suggestion_for("clear", "windows"), "Clear-Host");
}

#[test]
fn translates_pwd_to_windows_powershell() {
    assert_eq!(first_suggestion_for("pwd", "windows"), "Get-Location");
}

#[test]
fn translates_open_current_directory_to_windows_powershell() {
    assert_eq!(first_suggestion_for("open .", "windows"), "Invoke-Item .");
}

#[test]
fn translates_grep_to_select_string_on_windows() {
    assert_eq!(
        first_suggestion_for("grep error log.txt", "windows"),
        "Select-String error log.txt"
    );
}

#[test]
fn translates_which_to_get_command_on_windows() {
    assert_eq!(
        first_suggestion_for("which git", "windows"),
        "Get-Command git"
    );
}

#[test]
fn translates_cat_to_get_content_on_windows() {
    assert_eq!(
        first_suggestion_for("cat file.txt", "windows"),
        "Get-Content file.txt"
    );
}

#[test]
fn translates_windows_ipconfig_all_to_macos_dns_command() {
    assert_eq!(
        suggestions_for("ipconfig /all", "macos"),
        vec!["scutil --dns"]
    );
}

#[test]
fn translates_mkdir_with_argument() {
    assert_eq!(
        suggestions_for("mkdir test-folder", "macos"),
        vec!["mkdir test-folder"]
    );
}
