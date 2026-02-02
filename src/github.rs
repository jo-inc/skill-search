use crate::db::{Database, Skill};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Registry {
    pub name: &'static str,
    pub repo_url: &'static str,
    pub skills_path: &'static str,
    pub trusted: bool,
}

pub const REGISTRIES: &[Registry] = &[
    Registry {
        name: "clawdhub",
        repo_url: "https://github.com/openclaw/skills.git",
        skills_path: "skills",
        trusted: false, // Community skills, need individual verification
    },
    Registry {
        name: "anthropic",
        repo_url: "https://github.com/anthropics/skills.git",
        skills_path: "skills",
        trusted: true, // Official Anthropic skills
    },
    Registry {
        name: "openai",
        repo_url: "https://github.com/openai/skills.git",
        skills_path: "skills/.curated",
        trusted: true, // Official OpenAI curated skills
    },
    Registry {
        name: "openai-experimental",
        repo_url: "https://github.com/openai/skills.git",
        skills_path: "skills/.experimental",
        trusted: false, // Experimental skills, not yet curated
    },
];

#[derive(Debug, Deserialize)]
struct ClawdhubSkill {
    slug: String,
    stats: ClawdhubStats,
}

#[derive(Debug, Deserialize)]
struct ClawdhubStats {
    stars: i64,
}

#[derive(Debug, Deserialize)]
struct ClawdhubResponse {
    items: Vec<ClawdhubSkill>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

pub async fn sync_all_registries(db: &mut Database, repos_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(repos_dir)?;

    for registry in REGISTRIES {
        tracing::info!("Syncing registry: {}", registry.name);
        if let Err(e) = sync_registry(db, repos_dir, registry).await {
            tracing::warn!("Failed to sync {}: {}", registry.name, e);
        }
    }

    // Fetch star counts from clawdhub API
    tracing::info!("Fetching star counts from clawdhub API...");
    if let Err(e) = fetch_clawdhub_stars(db).await {
        tracing::warn!("Failed to fetch clawdhub stars: {}", e);
    }

    Ok(())
}

async fn fetch_clawdhub_stars(db: &mut Database) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("skill-search/0.1")
        .build()?;

    let mut stars_map: HashMap<String, i64> = HashMap::new();
    let mut cursor: Option<String> = None;
    let mut page = 0;

    loop {
        let url = match &cursor {
            Some(c) => format!("https://clawhub.com/api/v1/skills?limit=100&cursor={}", c),
            None => "https://clawhub.com/api/v1/skills?limit=100".to_string(),
        };

        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Clawdhub API error: {}", resp.status());
        }

        let data: ClawdhubResponse = resp.json().await?;
        
        for skill in data.items {
            stars_map.insert(skill.slug, skill.stats.stars);
        }

        page += 1;
        if page % 10 == 0 {
            tracing::debug!("Fetched {} skills from clawdhub API", stars_map.len());
        }

        match data.next_cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }

    tracing::info!("Fetched stars for {} clawdhub skills", stars_map.len());

    // Update stars in database
    for (slug, stars) in stars_map {
        db.update_stars("clawdhub", &slug, stars)?;
    }

    Ok(())
}

async fn sync_registry(db: &mut Database, repos_dir: &Path, registry: &Registry) -> Result<()> {
    let repo_dir = repos_dir.join(registry.name);

    // Clone or pull
    if repo_dir.join(".git").exists() {
        tracing::info!("Pulling updates for {}", registry.name);
        let status = Command::new("git")
            .args(["pull", "--ff-only", "-q"])
            .current_dir(&repo_dir)
            .status()?;
        if !status.success() {
            tracing::warn!("git pull failed for {}, trying fresh clone", registry.name);
            std::fs::remove_dir_all(&repo_dir)?;
            clone_repo(registry.repo_url, &repo_dir)?;
        }
    } else {
        clone_repo(registry.repo_url, &repo_dir)?;
    }

    // Scan for skills
    let skills_dir = repo_dir.join(registry.skills_path);
    if !skills_dir.exists() {
        anyhow::bail!("Skills directory not found: {:?}", skills_dir);
    }

    scan_skills_dir(db, registry, &skills_dir, &repo_dir)?;
    
    // Count skills
    let mut count = 0;
    for entry in std::fs::read_dir(&skills_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            count += 1;
        }
    }

    tracing::info!("Synced {} skills from {}", count, registry.name);

    // Update sync state
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    db.set_last_sync(registry.name, now, None)?;

    Ok(())
}

fn clone_repo(url: &str, dest: &Path) -> Result<()> {
    tracing::info!("Cloning {} to {:?}", url, dest);
    let status = Command::new("git")
        .args(["clone", "--depth", "1", "-q", url])
        .arg(dest)
        .status()?;
    if !status.success() {
        anyhow::bail!("git clone failed");
    }
    Ok(())
}

fn scan_skills_dir(db: &mut Database, registry: &Registry, dir: &Path, repo_root: &Path) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if !path.is_dir() {
            continue;
        }

        let skill_md_path = path.join("SKILL.md");
        if skill_md_path.exists() {
            if let Err(e) = process_skill(db, registry, &path, &skill_md_path, repo_root) {
                tracing::debug!("Skipping {:?}: {}", path, e);
            }
        } else {
            // Check subdirectories (for nested structure like clawdhub's author/skill)
            if let Ok(entries) = std::fs::read_dir(&path) {
                for sub in entries.flatten() {
                    let sub_path = sub.path();
                    if sub_path.is_dir() {
                        let sub_skill_md = sub_path.join("SKILL.md");
                        if sub_skill_md.exists() {
                            if let Err(e) = process_skill(db, registry, &sub_path, &sub_skill_md, repo_root) {
                                tracing::debug!("Skipping {:?}: {}", sub_path, e);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn process_skill(db: &mut Database, registry: &Registry, skill_dir: &Path, skill_md_path: &Path, repo_root: &Path) -> Result<()> {
    let skill_md = std::fs::read_to_string(skill_md_path)?;
    let (name, description, version) = parse_skill_frontmatter(&skill_md);

    // Extract slug from directory name
    let slug = skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Build GitHub URL from relative path
    let rel_path = skill_dir.strip_prefix(repo_root).unwrap_or(skill_dir);
    let github_url = match registry.name {
        "clawdhub" => format!("https://github.com/openclaw/skills/tree/main/{}", rel_path.display()),
        "anthropic" => format!("https://github.com/anthropics/skills/tree/main/{}", rel_path.display()),
        "openai" | "openai-experimental" => format!("https://github.com/openai/skills/tree/main/{}", rel_path.display()),
        _ => format!("https://github.com/unknown/{}", rel_path.display()),
    };

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let skill = Skill {
        id: 0,
        slug,
        name,
        registry: registry.name.to_string(),
        description,
        skill_md,
        github_url,
        version,
        stars: 0, // Will be updated from clawdhub API
        trusted: registry.trusted,
        updated_at: now,
    };

    db.upsert_skill(&skill)?;
    Ok(())
}

pub fn parse_skill_frontmatter(content: &str) -> (String, String, Option<String>) {
    let mut name = String::new();
    let mut description = String::new();
    let mut version = None;

    if content.starts_with("---") {
        if let Some(end_idx) = content[3..].find("---") {
            let frontmatter = &content[3..3 + end_idx];

            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("name:") {
                    name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = line.strip_prefix("description:") {
                    description = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = line.strip_prefix("version:") {
                    version = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
                }
            }
        }
    }

    // Fallback: use first heading as name
    if name.is_empty() {
        for line in content.lines() {
            if let Some(heading) = line.strip_prefix("# ") {
                name = heading.trim().to_string();
                break;
            }
        }
    }

    (name, description, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_complete() {
        let content = r#"---
name: test-skill
description: A test skill for testing
version: 1.0.0
---

# Test Skill

Some content here.
"#;
        let (name, description, version) = parse_skill_frontmatter(content);
        assert_eq!(name, "test-skill");
        assert_eq!(description, "A test skill for testing");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = r#"---
name: "quoted-skill"
description: 'Single quoted description'
version: "2.0"
---
"#;
        let (name, description, version) = parse_skill_frontmatter(content);
        assert_eq!(name, "quoted-skill");
        assert_eq!(description, "Single quoted description");
        assert_eq!(version, Some("2.0".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_no_version() {
        let content = r#"---
name: simple-skill
description: Just a simple skill
---
"#;
        let (name, description, version) = parse_skill_frontmatter(content);
        assert_eq!(name, "simple-skill");
        assert_eq!(description, "Just a simple skill");
        assert!(version.is_none());
    }

    #[test]
    fn test_parse_frontmatter_fallback_to_heading() {
        let content = r#"# My Cool Skill

This skill does cool things.
"#;
        let (name, description, version) = parse_skill_frontmatter(content);
        assert_eq!(name, "My Cool Skill");
        assert_eq!(description, "");
        assert!(version.is_none());
    }

    #[test]
    fn test_parse_frontmatter_empty_content() {
        let content = "";
        let (name, description, version) = parse_skill_frontmatter(content);
        assert_eq!(name, "");
        assert_eq!(description, "");
        assert!(version.is_none());
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter_with_heading() {
        let content = "Some text before\n# The Heading\nMore content";
        let (name, description, version) = parse_skill_frontmatter(content);
        assert_eq!(name, "The Heading");
        assert_eq!(description, "");
        assert!(version.is_none());
    }

    #[test]
    fn test_registries_configuration() {
        assert_eq!(REGISTRIES.len(), 4);
        
        let clawdhub = &REGISTRIES[0];
        assert_eq!(clawdhub.name, "clawdhub");
        assert!(!clawdhub.trusted);
        
        let anthropic = &REGISTRIES[1];
        assert_eq!(anthropic.name, "anthropic");
        assert!(anthropic.trusted);
        
        let openai = &REGISTRIES[2];
        assert_eq!(openai.name, "openai");
        assert!(openai.trusted);
        
        let openai_exp = &REGISTRIES[3];
        assert_eq!(openai_exp.name, "openai-experimental");
        assert!(!openai_exp.trusted);
    }
}
