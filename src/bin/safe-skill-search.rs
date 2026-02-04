mod db {
    include!("../db.rs");
}
mod github {
    include!("../github.rs");
}
mod index {
    include!("../index.rs");
}
mod quality {
    include!("../quality.rs");
}
mod skillssh {
    include!("../skillssh.rs");
}

use anyhow::Result;
use clap::{Parser, Subcommand};
use quality::QualityScores;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "safe-skill-search")]
#[command(about = "Search skills with quality filtering - only returns high-quality skills (score >= 80) by default")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Data directory (default: ~/.local/share/skill-search/)
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync skills from all registries
    Sync {
        /// Force full resync (ignore cache)
        #[arg(long)]
        force: bool,
    },
    /// Search for skills
    Search {
        /// Search query
        query: String,

        /// Number of results (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Filter by registry (clawdhub, anthropic, openai)
        #[arg(short, long)]
        registry: Option<String>,

        /// Only show trusted skills (anthropic, openai)
        #[arg(long)]
        trusted: bool,

        /// Minimum quality score (default: 80, set to 0 to show all)
        #[arg(long, default_value = "80")]
        min_score: i64,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show skill details
    Show {
        /// Skill slug
        slug: String,
    },
    /// Get install URL for a skill
    Url {
        /// Skill slug
        slug: String,
    },
    /// List top skills by stars
    Top {
        /// Number of results (default: 20)
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Only show trusted skills
        #[arg(long)]
        trusted: bool,

        /// Minimum quality score (default: 80, set to 0 to show all)
        #[arg(long, default_value = "80")]
        min_score: i64,
    },
}

fn get_data_dir(cli_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = cli_path {
        return Ok(p);
    }
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
    let data_dir = PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("skill-search");
    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("skill_search=debug,info")
    } else {
        EnvFilter::new("skill_search=info,warn")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let data_dir = get_data_dir(cli.data_dir)?;
    let db_path = data_dir.join("skills.db");
    let index_path = data_dir.join("index");
    let repos_dir = data_dir.join("repos");

    let mut db = db::Database::open(&db_path)?;
    let search_index = index::SearchIndex::open_or_create(&index_path)?;
    let quality_scores = QualityScores::load();

    // Auto-sync on first launch
    if db.needs_initial_sync()? {
        tracing::info!("First launch detected, syncing skills...");
        github::sync_all_registries(&mut db, &repos_dir).await?;
        skillssh::sync_skillssh(&mut db).await?;
        search_index.rebuild(&db)?;
    }

    match cli.command {
        Commands::Sync { force } => {
            if force {
                db.clear_sync_state()?;
            }
            github::sync_all_registries(&mut db, &repos_dir).await?;
            skillssh::sync_skillssh(&mut db).await?;
            search_index.rebuild(&db)?;
            tracing::info!("Sync complete");
        }
        Commands::Search {
            query,
            limit,
            registry,
            trusted,
            min_score,
            json,
        } => {
            let results = search_index.search(&query, limit * 4, registry.as_deref())?;

            let enriched: Vec<_> = results
                .into_iter()
                .filter_map(|r| {
                    db.get_skill(&r.registry, &r.slug).ok().flatten().map(|s| {
                        let quality_score = quality_scores
                            .get_score(&s.registry, &s.slug)
                            .or_else(|| quality_scores.get_score(&s.registry, &s.name))
                            .unwrap_or(0);
                        
                        serde_json::json!({
                            "slug": s.slug,
                            "name": s.name,
                            "registry": s.registry,
                            "description": s.description,
                            "github_url": s.github_url,
                            "stars": s.stars,
                            "trusted": s.trusted,
                            "search_score": r.score,
                            "quality_score": quality_score,
                        })
                    })
                })
                .filter(|r| !trusted || r["trusted"].as_bool().unwrap_or(false))
                .filter(|r| r["quality_score"].as_i64().unwrap_or(0) >= min_score)
                .take(limit)
                .collect();

            if json {
                println!("{}", serde_json::to_string_pretty(&enriched)?);
            } else {
                if enriched.is_empty() {
                    println!("No skills found with score >= {}. Try --min-score 0 to see all.", min_score);
                } else {
                    for (i, r) in enriched.iter().enumerate() {
                        let trusted = r["trusted"].as_bool().unwrap_or(false);
                        let trust_icon = if trusted { "✓" } else { "⚠" };
                        let stars = r["stars"].as_i64().unwrap_or(0);
                        let quality = r["quality_score"].as_i64().unwrap_or(0);
                        let stars_str = if stars > 0 { format!(" ★{}", stars) } else { String::new() };
                        
                        println!(
                            "{}. [{}] {}{} ({}) [Q:{}] - {}",
                            i + 1,
                            trust_icon,
                            r["name"].as_str().unwrap_or(""),
                            stars_str,
                            r["registry"].as_str().unwrap_or(""),
                            quality,
                            r["description"].as_str().unwrap_or("")
                        );
                        println!("   {}", r["github_url"].as_str().unwrap_or(""));
                        println!();
                    }
                }
            }
        }
        Commands::Show { slug } => {
            let skill = db.get_skill_by_slug(&slug)?;
            match skill {
                Some(s) => {
                    let quality_score = quality_scores
                        .get_score(&s.registry, &s.slug)
                        .or_else(|| quality_scores.get_score(&s.registry, &s.name))
                        .unwrap_or(0);
                    
                    println!("Name: {}", s.name);
                    println!("Registry: {}", s.registry);
                    println!("Trusted: {}", if s.trusted { "yes" } else { "no" });
                    println!("Stars: {}", s.stars);
                    println!("Quality Score: {}", quality_score);
                    println!("Description: {}", s.description);
                    println!("URL: {}", s.github_url);
                    if !s.skill_md.is_empty() {
                        println!("\n--- SKILL.md ---\n{}", s.skill_md);
                    }
                }
                None => {
                    eprintln!("Skill not found: {}", slug);
                    std::process::exit(1);
                }
            }
        }
        Commands::Url { slug } => {
            let skill = db.get_skill_by_slug(&slug)?;
            match skill {
                Some(s) => println!("{}", s.github_url),
                None => {
                    eprintln!("Skill not found: {}", slug);
                    std::process::exit(1);
                }
            }
        }
        Commands::Top { limit, trusted, min_score } => {
            let all_skills = db.get_all_skills()?;
            let mut skills: Vec<_> = all_skills
                .into_iter()
                .filter(|s| !trusted || s.trusted)
                .filter_map(|s| {
                    let quality_score = quality_scores
                        .get_score(&s.registry, &s.slug)
                        .or_else(|| quality_scores.get_score(&s.registry, &s.name))
                        .unwrap_or(0);
                    
                    if quality_score >= min_score {
                        Some((s, quality_score))
                    } else {
                        None
                    }
                })
                .collect();
            
            skills.sort_by(|a, b| b.0.stars.cmp(&a.0.stars));

            if skills.is_empty() {
                println!("No skills found with score >= {}. Try --min-score 0 to see all.", min_score);
            } else {
                for (i, (s, quality_score)) in skills.iter().take(limit).enumerate() {
                    let trust_icon = if s.trusted { "✓" } else { "⚠" };
                    println!(
                        "{}. [{}] {} ★{} ({}) [Q:{}] - {}",
                        i + 1,
                        trust_icon,
                        s.name,
                        s.stars,
                        s.registry,
                        quality_score,
                        s.description
                    );
                }
            }
        }
    }

    Ok(())
}
