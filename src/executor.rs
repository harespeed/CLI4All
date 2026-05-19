use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::platform::Platform;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub status_code: Option<i32>,
}

pub trait CommandExecutor {
    fn execute(&self, command: &str) -> Result<ExecutionResult>;
}

#[derive(Debug, Clone, Copy)]
pub struct SystemCommandExecutor {
    platform: Platform,
}

impl SystemCommandExecutor {
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }
}

impl CommandExecutor for SystemCommandExecutor {
    fn execute(&self, command: &str) -> Result<ExecutionResult> {
        if command.contains('\n') || command.contains('\r') {
            bail!("shell mode only supports single-line translated commands");
        }

        let output = match self.platform {
            Platform::Macos => Command::new("/bin/zsh")
                .args(["-c", command])
                .output()
                .context("failed to execute translated command with /bin/zsh")?,
            Platform::Ubuntu => Command::new("/bin/bash")
                .args(["-c", command])
                .output()
                .context("failed to execute translated command with /bin/bash")?,
            Platform::Windows => Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", command])
                .output()
                .context(
                    "failed to execute translated command with PowerShell; Phase 1 requires powershell.exe on Windows",
                )?,
        };

        Ok(ExecutionResult {
            command: command.to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status_code: output.status.code(),
        })
    }
}
