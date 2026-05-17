use crate::detector::detect_command;
use crate::rules::CommandCatalog;
use crate::translator::display_platform_name;

#[derive(Debug, Clone)]
pub struct FixResult {
    pub command: String,
    pub source: String,
    pub suggestions: Vec<String>,
}

pub fn suggest_fix(input: &str, catalog: &CommandCatalog) -> Option<FixResult> {
    detect_command(input, catalog).map(|detection| FixResult {
        command: detection.command,
        source: display_platform_name(&detection.source_platform).to_string(),
        suggestions: detection
            .intent
            .ubuntu_commands()
            .iter()
            .map(|command| {
                detection
                    .captures
                    .iter()
                    .fold(command.clone(), |value, (name, captured)| {
                        value.replace(&format!("<{name}>"), captured)
                    })
            })
            .collect(),
    })
}
