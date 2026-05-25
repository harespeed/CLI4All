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
fn translates_windows_dir_with_quoted_path_to_macos_ls_path() {
    assert_eq!(
        suggestions_for(r#"dir "Program Files""#, "macos"),
        vec![r#"ls "Program Files""#.to_string(), "ls".to_string()]
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
fn translates_copy_with_quoted_paths_to_macos_cp() {
    assert_eq!(
        suggestions_for(r#"copy "old name.txt" "new name.txt""#, "macos"),
        vec![r#"cp "old name.txt" "new name.txt""#]
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
fn translates_findstr_with_quoted_pattern_and_file() {
    assert_eq!(
        suggestions_for(r#"findstr "error code" "app log.txt""#, "macos"),
        vec![r#"grep "error code" "app log.txt""#]
    );
}

#[test]
fn translates_where_to_which_on_macos() {
    assert_eq!(
        suggestions_for("where git", "macos"),
        vec!["which git", "command -v git"]
    );
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
        vec!["netstat -an", "lsof -i -P -n | grep LISTEN"]
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
fn translates_unix_ls_hidden_with_path_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for(r#"ls -a "Program Files""#, "windows"),
        r#"Get-ChildItem -Force "Program Files""#
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
fn translates_open_quoted_path_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for(r#"open "My Folder""#, "windows"),
        r#"Start-Process "My Folder""#
    );
}

#[test]
fn translates_grep_to_select_string_on_windows() {
    assert_eq!(
        first_suggestion_for("grep error log.txt", "windows"),
        "Select-String -CaseSensitive error log.txt"
    );
}

#[test]
fn translates_grep_ignore_case_to_select_string_on_windows() {
    assert_eq!(
        first_suggestion_for(r#"grep -i "error code" "app log.txt""#, "windows"),
        r#"Select-String "error code" "app log.txt""#
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
fn translates_windows_ping_count_to_unix_ping_count() {
    assert_eq!(
        first_suggestion_for("ping -n 2 example.com", "macos"),
        "ping -c 2 example.com"
    );
}

#[test]
fn translates_unix_ping_count_to_windows_test_connection() {
    assert_eq!(
        first_suggestion_for("ping -c 3 example.com", "windows"),
        "Test-Connection -Count 3 example.com"
    );
}

#[test]
fn translates_mkdir_with_argument() {
    assert_eq!(
        suggestions_for("mkdir test-folder", "macos"),
        vec!["mkdir test-folder"]
    );
}

#[test]
fn translates_clip_with_quoted_text_to_macos_pbcopy() {
    assert_eq!(
        suggestions_for(r#"clip "hello world""#, "macos"),
        vec![r#"printf %s "hello world" | pbcopy"#]
    );
}

#[test]
fn translates_findstr_ignore_case_to_grep_ignore_case() {
    assert_eq!(
        first_suggestion_for(r#"findstr /i "error code" "app log.txt""#, "macos"),
        r#"grep -i "error code" "app log.txt""#
    );
}

#[test]
fn translates_head_with_count_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for("head -n 20 app.log", "windows"),
        "Get-Content app.log -TotalCount 20"
    );
}

#[test]
fn translates_tail_with_count_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for("tail -n 50 app.log", "windows"),
        "Get-Content app.log -Tail 50"
    );
}

#[test]
fn translates_recursive_grep_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for(r#"grep -R "TODO" ."#, "windows"),
        r#"Select-String -Path . -Pattern "TODO" -Recurse"#
    );
}

#[test]
fn translates_find_by_name_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for(r#"find . -name "*.rs""#, "windows"),
        r#"Get-ChildItem -Path . -Recurse -Filter "*.rs""#
    );
}

#[test]
fn translates_tracert_to_macos_traceroute() {
    assert_eq!(
        first_suggestion_for("tracert example.com", "macos"),
        "traceroute example.com"
    );
}

#[test]
fn translates_nslookup_to_windows_dns_lookup() {
    assert_eq!(
        first_suggestion_for("nslookup example.com", "windows"),
        "Resolve-DnsName example.com"
    );
}

#[test]
fn translates_curl_head_to_windows_invoke_webrequest() {
    assert_eq!(
        first_suggestion_for("curl -I https://example.com", "windows"),
        "Invoke-WebRequest -Method Head https://example.com"
    );
}

#[test]
fn translates_curl_download_to_windows_invoke_webrequest() {
    assert_eq!(
        first_suggestion_for(
            "curl -L https://example.com/file.zip -o file.zip",
            "windows"
        ),
        "Invoke-WebRequest https://example.com/file.zip -OutFile file.zip"
    );
}

#[test]
fn translates_listen_ports_to_macos_lsof() {
    assert_eq!(
        first_suggestion_for("Get-NetTCPConnection -State Listen", "macos"),
        "lsof -i -P -n | grep LISTEN"
    );
}

#[test]
fn translates_process_by_port_to_windows_powershell() {
    assert_eq!(
        first_suggestion_for("lsof -i :3000", "windows"),
        "Get-NetTCPConnection -LocalPort 3000"
    );
}

#[test]
fn translates_zip_creation_to_windows_compress_archive() {
    assert_eq!(
        first_suggestion_for("zip -r app.zip app", "windows"),
        "Compress-Archive -Path app -DestinationPath app.zip"
    );
}

#[test]
fn translates_unzip_to_windows_expand_archive() {
    assert_eq!(
        first_suggestion_for("unzip app.zip", "windows"),
        "Expand-Archive -Path app.zip -DestinationPath ."
    );
}

#[test]
fn translates_where_cargo_to_macos_which() {
    assert_eq!(first_suggestion_for("where cargo", "macos"), "which cargo");
}

#[test]
fn keeps_git_status_identical_across_platforms() {
    assert_eq!(first_suggestion_for("git status", "windows"), "git status");
}
