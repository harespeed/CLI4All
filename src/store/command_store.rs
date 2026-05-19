use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRecord {
    pub intent: String,
    pub description: String,
    pub category: String,
    pub risk_level: String,
    pub commands: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl CommandRecord {
    pub fn ubuntu_commands(&self) -> &[String] {
        self.platform_commands("ubuntu")
    }

    pub fn platform_commands(&self, platform: &str) -> &[String] {
        self.commands
            .get(platform)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn target_commands(&self, target: &str) -> &[String] {
        match target {
            "ubuntu" => self.platform_commands("ubuntu"),
            "macos" => self.platform_commands("macos"),
            "windows" => {
                let powershell = self.platform_commands("powershell");
                if powershell.is_empty() {
                    self.platform_commands("windows_cmd")
                } else {
                    powershell
                }
            }
            "powershell" => self.platform_commands("powershell"),
            "windows_cmd" => self.platform_commands("windows_cmd"),
            _ => &[],
        }
    }

    pub fn iter_commands(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.commands
            .iter()
            .map(|(platform, commands)| (platform.as_str(), commands.as_slice()))
    }
}

pub trait CommandStore {
    fn find_by_command(&self, command: &str) -> Result<Option<CommandRecord>>;
    fn find_by_intent(&self, intent: &str) -> Result<Option<CommandRecord>>;
    fn list_by_category(&self, category: &str) -> Result<Vec<CommandRecord>>;
}
