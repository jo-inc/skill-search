use crate::db::Database;
use anyhow::Result;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::{IndexRecordOption, Schema, STORED, STRING, TEXT, Field, Value};
use tantivy::{Index, IndexWriter, Term, TantivyDocument};

pub struct SearchIndex {
    index: Index,
    #[allow(dead_code)]
    schema: Schema,
    slug_field: Field,
    name_field: Field,
    description_field: Field,
    content_field: Field,
    registry_field: Field,
}

impl SearchIndex {
    pub fn open_or_create(index_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(index_path)?;

        let mut schema_builder = Schema::builder();
        let slug_field = schema_builder.add_text_field("slug", TEXT | STORED);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let description_field = schema_builder.add_text_field("description", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let registry_field = schema_builder.add_text_field("registry", STRING | STORED);
        let schema = schema_builder.build();

        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path)?
        } else {
            Index::create_in_dir(index_path, schema.clone())?
        };

        Ok(Self {
            index,
            schema,
            slug_field,
            name_field,
            description_field,
            content_field,
            registry_field,
        })
    }

    pub fn rebuild(&self, db: &Database) -> Result<()> {
        let mut index_writer: IndexWriter = self.index.writer(50_000_000)?;
        index_writer.delete_all_documents()?;

        let skills = db.get_all_skills()?;
        tracing::info!("Indexing {} skills", skills.len());

        for skill in skills {
            let mut doc = TantivyDocument::new();
            doc.add_text(self.slug_field, &skill.slug);
            doc.add_text(self.name_field, &skill.name);
            doc.add_text(self.description_field, &skill.description);
            doc.add_text(self.registry_field, &skill.registry);
            // Combine name, description, and skill_md for full-text search
            let content = format!("{} {} {}", skill.name, skill.description, skill.skill_md);
            doc.add_text(self.content_field, &content);
            index_writer.add_document(doc)?;
        }

        index_writer.commit()?;
        tracing::info!("Index rebuilt");
        Ok(())
    }

    pub fn search(&self, query_str: &str, limit: usize, registry: Option<&str>) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.name_field, self.description_field, self.content_field],
        );
        let text_query = query_parser.parse_query(query_str)?;

        // Build final query with optional registry filter
        let final_query: Box<dyn tantivy::query::Query> = if let Some(reg) = registry {
            let registry_term = Term::from_field_text(self.registry_field, reg);
            let registry_query = TermQuery::new(registry_term, IndexRecordOption::Basic);
            Box::new(BooleanQuery::new(vec![
                (Occur::Must, text_query),
                (Occur::Must, Box::new(registry_query)),
            ]))
        } else {
            text_query
        };

        let top_docs = searcher.search(&*final_query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            
            let slug = doc.get_first(self.slug_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = doc.get_first(self.name_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = doc.get_first(self.description_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let registry = doc.get_first(self.registry_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                slug,
                name,
                description,
                registry,
                score,
            });
        }

        Ok(results)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub registry: String,
    pub score: f32,
}

impl SearchResult {
    pub fn unique_key(&self) -> String {
        format!("{}:{}", self.registry, self.slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, Skill};
    use tempfile::tempdir;

    fn create_test_skill(slug: &str, name: &str, description: &str, registry: &str) -> Skill {
        Skill {
            id: 0,
            slug: slug.to_string(),
            name: name.to_string(),
            registry: registry.to_string(),
            description: description.to_string(),
            skill_md: format!("# {}\n\n{}", name, description),
            github_url: format!("https://github.com/test/{}", slug),
            version: Some("1.0.0".to_string()),
            stars: 0,
            trusted: registry == "anthropic",
            updated_at: 1234567890,
        }
    }

    #[test]
    fn test_search_index_create() {
        let dir = tempdir().unwrap();
        let index_path = dir.path().join("index");
        let _index = SearchIndex::open_or_create(&index_path).unwrap();
        assert!(index_path.join("meta.json").exists());
    }

    #[test]
    fn test_search_index_rebuild_and_search() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let index_path = dir.path().join("index");

        let db = Database::open(&db_path).unwrap();
        db.upsert_skill(&create_test_skill("calendar", "Calendar Manager", "Manage your calendar events", "clawdhub")).unwrap();
        db.upsert_skill(&create_test_skill("pdf-reader", "PDF Reader", "Read and extract PDF content", "anthropic")).unwrap();
        db.upsert_skill(&create_test_skill("browser", "Browser Automation", "Automate browser tasks", "openai")).unwrap();

        let index = SearchIndex::open_or_create(&index_path).unwrap();
        index.rebuild(&db).unwrap();

        let results = index.search("calendar", 10, None).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].slug, "calendar");
    }

    #[test]
    fn test_search_with_registry_filter() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let index_path = dir.path().join("index");

        let db = Database::open(&db_path).unwrap();
        db.upsert_skill(&create_test_skill("skill1", "Test Skill One", "A test skill", "clawdhub")).unwrap();
        db.upsert_skill(&create_test_skill("skill2", "Test Skill Two", "Another test skill", "anthropic")).unwrap();

        let index = SearchIndex::open_or_create(&index_path).unwrap();
        index.rebuild(&db).unwrap();

        let results = index.search("test skill", 10, Some("anthropic")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].registry, "anthropic");
    }

    #[test]
    fn test_search_no_results() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let index_path = dir.path().join("index");

        let db = Database::open(&db_path).unwrap();
        db.upsert_skill(&create_test_skill("calendar", "Calendar", "Calendar app", "clawdhub")).unwrap();

        let index = SearchIndex::open_or_create(&index_path).unwrap();
        index.rebuild(&db).unwrap();

        let results = index.search("nonexistent xyz abc", 10, None).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_result_unique_key() {
        let result = SearchResult {
            slug: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            description: "A test".to_string(),
            registry: "clawdhub".to_string(),
            score: 1.0,
        };
        assert_eq!(result.unique_key(), "clawdhub:test-skill");
    }

    #[test]
    fn test_search_respects_limit() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let index_path = dir.path().join("index");

        let db = Database::open(&db_path).unwrap();
        for i in 0..10 {
            db.upsert_skill(&create_test_skill(
                &format!("skill{}", i),
                &format!("Test Skill {}", i),
                "A test skill for testing",
                "clawdhub"
            )).unwrap();
        }

        let index = SearchIndex::open_or_create(&index_path).unwrap();
        index.rebuild(&db).unwrap();

        let results = index.search("test skill", 3, None).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_content_includes_skill_md() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let index_path = dir.path().join("index");

        let db = Database::open(&db_path).unwrap();
        let mut skill = create_test_skill("unique", "Generic Name", "Generic description", "clawdhub");
        skill.skill_md = "# Unique Skill\n\nThis skill handles XYZABC123 tasks.".to_string();
        db.upsert_skill(&skill).unwrap();

        let index = SearchIndex::open_or_create(&index_path).unwrap();
        index.rebuild(&db).unwrap();

        let results = index.search("XYZABC123", 10, None).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].slug, "unique");
    }
}
