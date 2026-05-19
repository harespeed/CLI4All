pub fn normalize_command(input: &str) -> String {
    input
        .split_whitespace()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn normalize_intent(intent: &str) -> String {
    intent.trim().to_ascii_lowercase()
}

pub fn normalize_category(category: &str) -> String {
    match category.trim().to_ascii_lowercase().as_str() {
        "networking" | "network" => "network".to_string(),
        other => other.replace(' ', "_"),
    }
}

pub fn exact_command_key(command: &str) -> String {
    format!("exact:{}", normalize_command(command))
}

pub fn intent_key(intent: &str) -> String {
    format!("intent:{}", normalize_intent(intent))
}

pub fn category_key(category: &str, intent: &str) -> String {
    format!(
        "category:{}:{}",
        normalize_category(category),
        normalize_intent(intent)
    )
}
