use anyhow::{anyhow, Result};
use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

pub const BUNDLED_DATA_DIR_ENV_VAR: &str = "CLI4ALL_BUNDLED_DATA_DIR";
const MANIFEST_DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");
const LOCAL_SHARE_RELATIVE_DIR: &str = ".local/share/cli4all/data";
const SYSTEM_DATA_DIRS: &[&str] = &[
    "/usr/local/share/cli4all/data",
    "/opt/homebrew/share/cli4all/data",
    "/usr/share/cli4all/data",
];

pub fn candidate_data_dirs() -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut dirs = Vec::new();

    if let Ok(value) = env::var("CLI4ALL_DATA_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            push_unique(&mut dirs, &mut seen, PathBuf::from(trimmed));
        }
    }

    if let Ok(cwd) = env::current_dir() {
        push_unique(&mut dirs, &mut seen, cwd.join("data"));
    }

    push_unique(&mut dirs, &mut seen, PathBuf::from(MANIFEST_DATA_DIR));

    if let Ok(value) = env::var(BUNDLED_DATA_DIR_ENV_VAR) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            push_unique(&mut dirs, &mut seen, PathBuf::from(trimmed));
        }
    }

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            push_unique(&mut dirs, &mut seen, exe_dir.join("data"));
        }
    }

    if let Some(home_dir) = home_dir() {
        push_unique(
            &mut dirs,
            &mut seen,
            home_dir.join(LOCAL_SHARE_RELATIVE_DIR),
        );
    }

    for path in SYSTEM_DATA_DIRS {
        push_unique(&mut dirs, &mut seen, PathBuf::from(path));
    }

    dirs
}

pub fn find_data_dir(required_files: &[&str]) -> Result<PathBuf> {
    let dirs = candidate_data_dirs();

    for dir in &dirs {
        if required_files
            .iter()
            .all(|file_name| dir.join(file_name).is_file())
        {
            return Ok(dir.clone());
        }
    }

    Err(anyhow!(format_missing_files_error(required_files, &dirs)))
}

pub fn find_data_file(file_name: &str) -> Result<PathBuf> {
    let dirs = candidate_data_dirs();

    for dir in &dirs {
        let path = dir.join(file_name);
        if path.is_file() {
            return Ok(path);
        }
    }

    Err(anyhow!(format_missing_files_error(&[file_name], &dirs)))
}

fn format_missing_files_error(required_files: &[&str], searched_dirs: &[PathBuf]) -> String {
    let header = format!(
        "CLI4ALL could not locate required runtime data file(s): {}",
        required_files.join(", ")
    );

    let searched = searched_dirs
        .iter()
        .map(|dir| {
            let missing = required_files
                .iter()
                .filter(|file_name| !dir.join(file_name).is_file())
                .copied()
                .collect::<Vec<_>>();

            if missing.is_empty() {
                format!("- {} (all files present but unavailable)", dir.display())
            } else {
                format!("- {} (missing: {})", dir.display(), missing.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{header}\nSearched directories:\n{searched}\nSet CLI4ALL_DATA_DIR to a directory containing these files, or reinstall CLI4ALL so the bundled runtime data is restored."
    )
}

fn push_unique(paths: &mut Vec<PathBuf>, seen: &mut BTreeSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}
