use crate::detector::detect_command;
use crate::rules::apply_template_captures;
use crate::store::CommandStore;
use crate::translator::display_platform_name;

#[derive(Debug, Clone)]
pub struct FixResult {
    pub command: String,
    pub source: String,
    pub suggestions: Vec<String>,
}

pub fn suggest_fix(input: &str, store: &impl CommandStore) -> anyhow::Result<Option<FixResult>> {
    Ok(detect_command(input, store)?.map(|detection| FixResult {
        command: detection.command,
        source: display_platform_name(&detection.source_platform).to_string(),
        suggestions: detection
            .intent
            .ubuntu_commands()
            .iter()
            .map(|command| apply_template_captures(command, &detection.captures))
            .collect(),
    }))
}
