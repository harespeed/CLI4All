use cli4all::rules::load_command_catalog;
use cli4all::translator::translate_command;

#[test]
fn translates_ipconfig_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("ipconfig", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("ipconfig should be known");

    assert_eq!(result.command, "ipconfig");
    assert_eq!(result.suggestions, vec!["ip addr", "hostname -I"]);
}

#[test]
fn translates_dir_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("dir", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("dir should be known");

    assert_eq!(result.command, "dir");
    assert_eq!(result.suggestions, vec!["ls", "ls -la"]);
}

#[test]
fn translates_cls_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("cls", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("cls should be known");

    assert_eq!(result.suggestions, vec!["clear"]);
}

#[test]
fn translates_tasklist_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("tasklist", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("tasklist should be known");

    assert_eq!(result.suggestions, vec!["ps aux"]);
}

#[test]
fn translates_where_python_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("where python", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("where should be known");

    assert_eq!(result.suggestions, vec!["which python"]);
}

#[test]
fn translates_netstat_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("netstat -ano", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("netstat should be known");

    assert_eq!(result.suggestions, vec!["ss -tulnp", "lsof -i"]);
}

#[test]
fn translates_brew_install_node_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("brew install node", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("brew should be known");

    assert_eq!(
        result.suggestions,
        vec!["sudo apt update && sudo apt install node"]
    );
}

#[test]
fn translates_del_file_to_ubuntu_equivalents() {
    let catalog = load_command_catalog().expect("command catalog should load");
    let result = translate_command("del test.txt", "ubuntu", &catalog)
        .expect("translation should succeed")
        .expect("del should be known");

    assert_eq!(result.suggestions, vec!["rm test.txt"]);
}
