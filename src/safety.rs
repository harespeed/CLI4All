use regex::Regex;

use crate::rules::RiskCatalog;

#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub level: String,
    pub reason: String,
    pub matched_rule: Option<String>,
}

pub fn assess_risk(input: &str, catalog: &RiskCatalog) -> RiskAssessment {
    let mut best_match = RiskAssessment {
        level: "none".to_string(),
        reason: "No known high-risk pattern matched.".to_string(),
        matched_rule: None,
    };
    let mut best_rank = 0;

    for rule in &catalog.risks {
        let matched = rule.patterns.iter().any(|pattern| {
            Regex::new(pattern)
                .expect("validated regex")
                .is_match(input)
        });

        if matched {
            let rank = severity_rank(&rule.level);
            if rank > best_rank {
                best_rank = rank;
                best_match = RiskAssessment {
                    level: rule.level.clone(),
                    reason: rule.reason.clone(),
                    matched_rule: Some(rule.name.clone()),
                };
            }
        }
    }

    best_match
}

fn severity_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "destructive" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}
