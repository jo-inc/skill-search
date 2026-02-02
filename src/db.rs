use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub registry: String,
    pub description: String,
    pub skill_md: String,
    pub github_url: String,
    pub version: Option<String>,
    pub stars: i64,
    pub trusted: bool,
    pub updated_at: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS skills (
                id INTEGER PRIMARY KEY,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                registry TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                skill_md TEXT NOT NULL DEFAULT '',
                github_url TEXT NOT NULL,
                version TEXT,
                stars INTEGER NOT NULL DEFAULT 0,
                trusted INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                UNIQUE(registry, slug)
            );

            CREATE INDEX IF NOT EXISTS idx_skills_slug ON skills(slug);
            CREATE INDEX IF NOT EXISTS idx_skills_registry ON skills(registry);
            CREATE INDEX IF NOT EXISTS idx_skills_stars ON skills(stars DESC);
            CREATE INDEX IF NOT EXISTS idx_skills_trusted ON skills(trusted);

            CREATE TABLE IF NOT EXISTS sync_state (
                registry TEXT PRIMARY KEY,
                last_sync INTEGER NOT NULL,
                etag TEXT
            );
            "#,
        )?;

        Ok(Self { conn })
    }

    pub fn needs_initial_sync(&self) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM skills", [], |row| row.get(0))?;
        Ok(count == 0)
    }

    pub fn clear_sync_state(&self) -> Result<()> {
        self.conn.execute("DELETE FROM sync_state", [])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_last_sync(&self, registry: &str) -> Result<Option<(i64, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT last_sync, etag FROM sync_state WHERE registry = ?")?;
        let result = stmt.query_row([registry], |row| Ok((row.get(0)?, row.get(1)?)));
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_last_sync(&self, registry: &str, timestamp: i64, etag: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_state (registry, last_sync, etag) VALUES (?, ?, ?)",
            params![registry, timestamp, etag],
        )?;
        Ok(())
    }

    pub fn upsert_skill(&self, skill: &Skill) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO skills (slug, name, registry, description, skill_md, github_url, version, stars, trusted, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(registry, slug) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                skill_md = excluded.skill_md,
                github_url = excluded.github_url,
                version = excluded.version,
                stars = excluded.stars,
                trusted = excluded.trusted,
                updated_at = excluded.updated_at
            "#,
            params![
                skill.slug,
                skill.name,
                skill.registry,
                skill.description,
                skill.skill_md,
                skill.github_url,
                skill.version,
                skill.stars,
                skill.trusted as i64,
                skill.updated_at,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        if id == 0 {
            let id: i64 = self.conn.query_row(
                "SELECT id FROM skills WHERE registry = ? AND slug = ?",
                params![skill.registry, skill.slug],
                |row| row.get(0),
            )?;
            Ok(id)
        } else {
            Ok(id)
        }
    }

    pub fn update_stars(&self, registry: &str, slug: &str, stars: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE skills SET stars = ? WHERE registry = ? AND slug = ?",
            params![stars, registry, slug],
        )?;
        Ok(())
    }

    pub fn get_skill(&self, registry: &str, slug: &str) -> Result<Option<Skill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, slug, name, registry, description, skill_md, github_url, version, stars, trusted, updated_at 
             FROM skills WHERE registry = ? AND slug = ? LIMIT 1",
        )?;
        let result = stmt.query_row(params![registry, slug], |row| {
            Ok(Skill {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                registry: row.get(3)?,
                description: row.get(4)?,
                skill_md: row.get(5)?,
                github_url: row.get(6)?,
                version: row.get(7)?,
                stars: row.get(8)?,
                trusted: row.get::<_, i64>(9)? != 0,
                updated_at: row.get(10)?,
            })
        });
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_skill_by_slug(&self, slug: &str) -> Result<Option<Skill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, slug, name, registry, description, skill_md, github_url, version, stars, trusted, updated_at 
             FROM skills WHERE slug = ? LIMIT 1",
        )?;
        let result = stmt.query_row([slug], |row| {
            Ok(Skill {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                registry: row.get(3)?,
                description: row.get(4)?,
                skill_md: row.get(5)?,
                github_url: row.get(6)?,
                version: row.get(7)?,
                stars: row.get(8)?,
                trusted: row.get::<_, i64>(9)? != 0,
                updated_at: row.get(10)?,
            })
        });
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_all_skills(&self) -> Result<Vec<Skill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, slug, name, registry, description, skill_md, github_url, version, stars, trusted, updated_at FROM skills",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Skill {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                registry: row.get(3)?,
                description: row.get(4)?,
                skill_md: row.get(5)?,
                github_url: row.get(6)?,
                version: row.get(7)?,
                stars: row.get(8)?,
                trusted: row.get::<_, i64>(9)? != 0,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    #[allow(dead_code)]
    pub fn get_clawdhub_slugs(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT slug FROM skills WHERE registry = 'clawdhub'")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    #[allow(dead_code)]
    pub fn get_skills_by_registry(&self, registry: &str) -> Result<Vec<Skill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, slug, name, registry, description, skill_md, github_url, version, stars, trusted, updated_at 
             FROM skills WHERE registry = ?",
        )?;
        let rows = stmt.query_map([registry], |row| {
            Ok(Skill {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                registry: row.get(3)?,
                description: row.get(4)?,
                skill_md: row.get(5)?,
                github_url: row.get(6)?,
                version: row.get(7)?,
                stars: row.get(8)?,
                trusted: row.get::<_, i64>(9)? != 0,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_skill(slug: &str, registry: &str, trusted: bool) -> Skill {
        Skill {
            id: 0,
            slug: slug.to_string(),
            name: format!("{} skill", slug),
            registry: registry.to_string(),
            description: format!("Description for {}", slug),
            skill_md: "# Test\nSome content".to_string(),
            github_url: format!("https://github.com/test/{}", slug),
            version: Some("1.0.0".to_string()),
            stars: 0,
            trusted,
            updated_at: 1234567890,
        }
    }

    #[test]
    fn test_database_open_creates_tables() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        assert!(db.needs_initial_sync().unwrap());
    }

    #[test]
    fn test_upsert_and_get_skill() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let skill = create_test_skill("test-skill", "clawdhub", false);
        let id = db.upsert_skill(&skill).unwrap();
        assert!(id > 0);

        let retrieved = db.get_skill("clawdhub", "test-skill").unwrap().unwrap();
        assert_eq!(retrieved.slug, "test-skill");
        assert_eq!(retrieved.name, "test-skill skill");
        assert_eq!(retrieved.registry, "clawdhub");
        assert!(!retrieved.trusted);
    }

    #[test]
    fn test_upsert_updates_existing_skill() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let mut skill = create_test_skill("update-test", "anthropic", true);
        db.upsert_skill(&skill).unwrap();

        skill.description = "Updated description".to_string();
        skill.stars = 100;
        db.upsert_skill(&skill).unwrap();

        let retrieved = db.get_skill("anthropic", "update-test").unwrap().unwrap();
        assert_eq!(retrieved.description, "Updated description");
        assert_eq!(retrieved.stars, 100);
    }

    #[test]
    fn test_get_skill_by_slug() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let skill = create_test_skill("slug-test", "openai", true);
        db.upsert_skill(&skill).unwrap();

        let retrieved = db.get_skill_by_slug("slug-test").unwrap().unwrap();
        assert_eq!(retrieved.slug, "slug-test");
        assert!(retrieved.trusted);
    }

    #[test]
    fn test_get_skill_by_slug_not_found() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let result = db.get_skill_by_slug("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_all_skills() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        db.upsert_skill(&create_test_skill("skill1", "clawdhub", false)).unwrap();
        db.upsert_skill(&create_test_skill("skill2", "anthropic", true)).unwrap();
        db.upsert_skill(&create_test_skill("skill3", "openai", true)).unwrap();

        let all_skills = db.get_all_skills().unwrap();
        assert_eq!(all_skills.len(), 3);
    }

    #[test]
    fn test_get_skills_by_registry() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        db.upsert_skill(&create_test_skill("skill1", "clawdhub", false)).unwrap();
        db.upsert_skill(&create_test_skill("skill2", "clawdhub", false)).unwrap();
        db.upsert_skill(&create_test_skill("skill3", "anthropic", true)).unwrap();

        let clawdhub_skills = db.get_skills_by_registry("clawdhub").unwrap();
        assert_eq!(clawdhub_skills.len(), 2);

        let anthropic_skills = db.get_skills_by_registry("anthropic").unwrap();
        assert_eq!(anthropic_skills.len(), 1);
    }

    #[test]
    fn test_update_stars() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let skill = create_test_skill("stars-test", "clawdhub", false);
        db.upsert_skill(&skill).unwrap();

        db.update_stars("clawdhub", "stars-test", 42).unwrap();

        let retrieved = db.get_skill("clawdhub", "stars-test").unwrap().unwrap();
        assert_eq!(retrieved.stars, 42);
    }

    #[test]
    fn test_needs_initial_sync() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        assert!(db.needs_initial_sync().unwrap());

        db.upsert_skill(&create_test_skill("skill1", "clawdhub", false)).unwrap();

        assert!(!db.needs_initial_sync().unwrap());
    }

    #[test]
    fn test_sync_state() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        assert!(db.get_last_sync("clawdhub").unwrap().is_none());

        db.set_last_sync("clawdhub", 1234567890, Some("etag123")).unwrap();

        let (timestamp, etag) = db.get_last_sync("clawdhub").unwrap().unwrap();
        assert_eq!(timestamp, 1234567890);
        assert_eq!(etag, Some("etag123".to_string()));
    }

    #[test]
    fn test_clear_sync_state() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        db.set_last_sync("clawdhub", 1234567890, None).unwrap();
        db.set_last_sync("anthropic", 1234567890, None).unwrap();

        db.clear_sync_state().unwrap();

        assert!(db.get_last_sync("clawdhub").unwrap().is_none());
        assert!(db.get_last_sync("anthropic").unwrap().is_none());
    }

    #[test]
    fn test_get_clawdhub_slugs() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        db.upsert_skill(&create_test_skill("skill1", "clawdhub", false)).unwrap();
        db.upsert_skill(&create_test_skill("skill2", "clawdhub", false)).unwrap();
        db.upsert_skill(&create_test_skill("skill3", "anthropic", true)).unwrap();

        let slugs = db.get_clawdhub_slugs().unwrap();
        assert_eq!(slugs.len(), 2);
        assert!(slugs.contains(&"skill1".to_string()));
        assert!(slugs.contains(&"skill2".to_string()));
    }
}
