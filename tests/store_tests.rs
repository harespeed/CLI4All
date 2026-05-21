use cli4all::store::{build_command_index, C4DbCommandStore, CommandStore};
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
            "cli4all-{label}-{}-{unique}-{counter}",
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

fn build_temp_store() -> C4DbCommandStore {
    let temp_dir = TempDir::new("store-tests");
    let index_path = temp_dir.path().join("commands.c4idx");
    let data_path = temp_dir.path().join("commands.c4dat");
    let source_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/commands.source.json"
    ));

    build_command_index(&source_path, &index_path, &data_path).expect("index build should work");
    C4DbCommandStore::open(&index_path, &data_path).expect("store should open")
}

#[test]
fn builds_index_and_data_from_json_source() {
    let temp_dir = TempDir::new("builder-output");
    let index_path = temp_dir.path().join("commands.c4idx");
    let data_path = temp_dir.path().join("commands.c4dat");
    let source_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/commands.source.json"
    ));

    build_command_index(&source_path, &index_path, &data_path).expect("index build should work");

    assert!(index_path.exists(), "index file should exist");
    assert!(data_path.exists(), "data file should exist");
    assert!(
        fs::metadata(index_path)
            .expect("index metadata should load")
            .len()
            > 0
    );
    assert!(
        fs::metadata(data_path)
            .expect("data metadata should load")
            .len()
            > 0
    );
}

#[test]
fn finds_ipconfig_by_exact_command() {
    let store = build_temp_store();
    let record = store
        .find_by_command("ipconfig")
        .expect("lookup should succeed")
        .expect("record should exist");

    assert_eq!(record.intent, "show_ip_config");
}

#[test]
fn finds_dir_by_exact_command() {
    let store = build_temp_store();
    let record = store
        .find_by_command("dir")
        .expect("lookup should succeed")
        .expect("record should exist");

    assert_eq!(record.intent, "list_files");
}

#[test]
fn finds_placeholder_command_by_template_scan() {
    let store = build_temp_store();
    let record = store
        .find_by_command("type file.txt")
        .expect("lookup should succeed")
        .expect("record should exist");

    assert_eq!(record.intent, "show_file_content");
}

#[test]
fn finds_quoted_placeholder_command_by_template_scan() {
    let store = build_temp_store();
    let record = store
        .find_by_command(r#"copy "old name.txt" "new name.txt""#)
        .expect("lookup should succeed")
        .expect("record should exist");

    assert_eq!(record.intent, "copy_file");
}

#[test]
fn finds_record_by_intent() {
    let store = build_temp_store();
    let record = store
        .find_by_intent("show_ip_config")
        .expect("intent lookup should succeed")
        .expect("record should exist");

    assert_eq!(record.intent, "show_ip_config");
    assert_eq!(record.risk_level, "low");
}

#[test]
fn lists_network_category_records() {
    let store = build_temp_store();
    let records = store
        .list_by_category("network")
        .expect("category lookup should succeed");

    assert!(
        records
            .iter()
            .any(|record| record.intent == "show_ip_config"),
        "show_ip_config should be listed under network"
    );
    assert!(
        records
            .iter()
            .any(|record| record.intent == "show_dns_config"),
        "show_dns_config should be listed under network"
    );
}
