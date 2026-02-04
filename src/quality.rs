use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct QualityEntry {
    pub name: String,
    pub registry: String,
    pub score: i64,
    #[allow(dead_code)]
    pub stars: i64,
    #[allow(dead_code)]
    pub rationale: String,
    pub url: String,
}

pub struct QualityScores {
    scores: HashMap<String, QualityEntry>,
}

impl QualityScores {
    pub fn load() -> Self {
        let json_data = include_str!("../skills.json");
        let entries: Vec<QualityEntry> = serde_json::from_str(json_data).unwrap_or_default();
        
        let mut scores = HashMap::new();
        for entry in entries {
            let key = format!("{}:{}", entry.registry, normalize_slug(&entry.name));
            scores.insert(key.clone(), entry.clone());
            let alt_key = format!("{}:{}", entry.registry, normalize_slug(&extract_slug_from_url(&entry.url)));
            if alt_key != key {
                scores.insert(alt_key, entry);
            }
        }
        
        Self { scores }
    }

    pub fn get_score(&self, registry: &str, slug: &str) -> Option<i64> {
        let key = format!("{}:{}", registry, normalize_slug(slug));
        self.scores.get(&key).map(|e| e.score)
    }

    #[allow(dead_code)]
    pub fn get_entry(&self, registry: &str, slug: &str) -> Option<&QualityEntry> {
        let key = format!("{}:{}", registry, normalize_slug(slug));
        self.scores.get(&key)
    }

    #[allow(dead_code)]
    pub fn all_entries(&self) -> impl Iterator<Item = &QualityEntry> {
        self.scores.values()
    }
}

fn normalize_slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn extract_slug_from_url(url: &str) -> String {
    url.rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_quality_scores() {
        let qs = QualityScores::load();
        assert!(qs.scores.len() > 0);
    }

    #[test]
    fn test_normalize_slug() {
        assert_eq!(normalize_slug("My-Skill"), "my-skill");
        assert_eq!(normalize_slug("SKILL_NAME"), "skill_name");
    }
}
