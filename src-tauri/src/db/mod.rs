use crate::models::{
    Category, DashboardStats, DuplicateAiAnalysis, DuplicateGroup, DuplicateRecord,
    DuplicatesOverview, Post, PublishLog, Source,
};
use std::collections::{BTreeMap, HashSet};
use crate::services::duplicate;
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        db.seed_categories()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hashtags TEXT NOT NULL DEFAULT '',
                keywords TEXT NOT NULL DEFAULT '',
                enabled INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                category_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_fetched_at TEXT,
                FOREIGN KEY (category_id) REFERENCES categories(id)
            );

            CREATE TABLE IF NOT EXISTS posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_url TEXT NOT NULL UNIQUE,
                raw_title TEXT NOT NULL,
                raw_description TEXT NOT NULL DEFAULT '',
                raw_image_url TEXT,
                ai_title TEXT,
                ai_text TEXT,
                ai_hashtags TEXT,
                category_id INTEGER,
                status TEXT NOT NULL DEFAULT 'new',
                vk_post_id TEXT,
                telegram_message_id TEXT,
                created_at TEXT NOT NULL,
                published_at TEXT,
                error_message TEXT,
                FOREIGN KEY (category_id) REFERENCES categories(id)
            );

            CREATE TABLE IF NOT EXISTS publish_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                post_id INTEGER NOT NULL,
                platform TEXT NOT NULL,
                success INTEGER NOT NULL,
                response TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (post_id) REFERENCES posts(id)
            );

            CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS parsed_items (
                normalized_url TEXT PRIMARY KEY,
                normalized_title TEXT NOT NULL DEFAULT '',
                source_url TEXT NOT NULL,
                seen_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS duplicate_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                duplicate_url TEXT NOT NULL,
                duplicate_title TEXT NOT NULL DEFAULT '',
                kept_post_id INTEGER,
                kept_title TEXT,
                reason TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )?;

        let _ = conn.execute("ALTER TABLE posts ADD COLUMN normalized_title TEXT", []);
        let _ = conn.execute("ALTER TABLE posts ADD COLUMN normalized_url TEXT", []);
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_posts_normalized_title ON posts(normalized_title)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_posts_normalized_url ON posts(normalized_url)",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE duplicate_records ADD COLUMN duplicate_description TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE duplicate_records ADD COLUMN ai_is_duplicate INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE duplicate_records ADD COLUMN ai_confidence INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE duplicate_records ADD COLUMN ai_explanation TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE duplicate_records ADD COLUMN ai_checked_at TEXT",
            [],
        );

        let mut stmt = conn.prepare(
            "SELECT id, source_url, raw_title FROM posts WHERE normalized_title IS NULL OR normalized_title = ''",
        )?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(Result::ok)
            .collect();

        for (id, url, title) in rows {
            let norm_title = duplicate::normalize_title(&title);
            let norm_url = duplicate::normalize_url(&url);
            conn.execute(
                "UPDATE posts SET normalized_title = ?1, normalized_url = ?2 WHERE id = ?3",
                params![norm_title, norm_url, id],
            )?;
        }

        conn.execute(
            "INSERT OR IGNORE INTO parsed_items (normalized_url, normalized_title, source_url, seen_at)
             SELECT normalized_url, normalized_title, source_url, created_at
             FROM posts
             WHERE normalized_url IS NOT NULL AND normalized_url != ''",
            [],
        )?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;

        conn.execute(
            "UPDATE posts SET status='new' WHERE status='processing'",
            [],
        )?;

        Ok(())
    }

    pub fn is_parsed(&self, source_url: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let norm_url = duplicate::normalize_url(source_url);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM parsed_items WHERE normalized_url = ?1",
            params![norm_url],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn is_url_seen(&self, source_url: &str) -> Result<bool> {
        if self.is_parsed(source_url)? {
            return Ok(true);
        }
        let conn = self.conn.lock().unwrap();
        let norm_url = duplicate::normalize_url(source_url);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE source_url = ?1 OR normalized_url = ?2",
            params![source_url, norm_url],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn record_parsed_item(&self, source_url: &str, raw_title: &str) -> Result<()> {
        self.record_parsed(source_url, raw_title)
    }

    fn record_parsed(&self, source_url: &str, raw_title: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let norm_url = duplicate::normalize_url(source_url);
        let norm_title = duplicate::normalize_title(raw_title);
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO parsed_items (normalized_url, normalized_title, source_url, seen_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![norm_url, norm_title, source_url, now],
        )?;
        Ok(())
    }

    fn seed_categories(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }

        let defaults = [
            ("PC", "#игры #PC #гейминг", "steam, pc, nvidia, amd"),
            ("Консоли", "#игры #консоли #PS5 #Xbox", "playstation, xbox, nintendo, switch"),
            ("Мобильные", "#игры #мобильные", "mobile, android, ios, gacha"),
            ("Киберспорт", "#киберспорт #esports", "esports, tournament, cs2, dota"),
            ("Инди", "#игры #инди", "indie, pixel, roguelike"),
            ("Анонсы", "#игры #анонс #релиз", "announce, release, trailer, reveal"),
            ("Обзоры", "#игры #обзор", "review, score, rating, gameplay"),
        ];

        for (name, hashtags, keywords) in defaults {
            conn.execute(
                "INSERT INTO categories (name, hashtags, keywords, enabled) VALUES (?1, ?2, ?3, 1)",
                params![name, hashtags, keywords],
            )?;
        }
        Ok(())
    }

    pub fn get_categories(&self) -> Result<Vec<Category>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, hashtags, keywords, enabled FROM categories ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                hashtags: row.get(2)?,
                keywords: row.get(3)?,
                enabled: row.get::<_, i32>(4)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_category(&self, category: &Category) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE categories SET name=?1, hashtags=?2, keywords=?3, enabled=?4 WHERE id=?5",
            params![
                category.name,
                category.hashtags,
                category.keywords,
                category.enabled as i32,
                category.id
            ],
        )?;
        Ok(())
    }

    pub fn get_category_by_name(&self, name: &str) -> Result<Option<Category>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, hashtags, keywords, enabled FROM categories WHERE name = ?1",
            params![name],
            |row| {
                Ok(Category {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    hashtags: row.get(2)?,
                    keywords: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_sources(&self) -> Result<Vec<Source>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, name, category_id, enabled, last_fetched_at FROM sources ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Source {
                id: row.get(0)?,
                url: row.get(1)?,
                name: row.get(2)?,
                category_id: row.get(3)?,
                enabled: row.get::<_, i32>(4)? != 0,
                last_fetched_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn add_source(
        &self,
        url: &str,
        name: &str,
        category_id: Option<i64>,
    ) -> Result<Source> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sources (url, name, category_id, enabled) VALUES (?1, ?2, ?3, 1)",
            params![url, name, category_id],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Source {
            id,
            url: url.to_string(),
            name: name.to_string(),
            category_id,
            enabled: true,
            last_fetched_at: None,
        })
    }

    pub fn update_source(&self, source: &Source) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sources SET url=?1, name=?2, category_id=?3, enabled=?4, last_fetched_at=?5 WHERE id=?6",
            params![
                source.url,
                source.name,
                source.category_id,
                source.enabled as i32,
                source.last_fetched_at,
                source.id
            ],
        )?;
        Ok(())
    }

    pub fn delete_source(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sources WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn source_exists(&self, url: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sources WHERE url = ?1",
            params![url],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn insert_post(
        &self,
        source_url: &str,
        raw_title: &str,
        raw_description: &str,
        raw_image_url: Option<&str>,
        category_id: Option<i64>,
    ) -> Result<i64> {
        self.insert_post_if_new(
            source_url,
            raw_title,
            raw_description,
            raw_image_url,
            category_id,
        )?
        .ok_or_else(|| anyhow::anyhow!("Post already exists: {}", source_url))
    }

    pub fn insert_post_if_new(
        &self,
        source_url: &str,
        raw_title: &str,
        raw_description: &str,
        raw_image_url: Option<&str>,
        category_id: Option<i64>,
    ) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        let norm_url = duplicate::normalize_url(source_url);
        let norm_title = duplicate::normalize_title(raw_title);
        let now = Utc::now().to_rfc3339();
        match conn.execute(
            "INSERT INTO posts (source_url, raw_title, raw_description, raw_image_url, category_id, status, created_at, normalized_title, normalized_url)
             VALUES (?1, ?2, ?3, ?4, ?5, 'new', ?6, ?7, ?8)",
            params![
                source_url,
                raw_title,
                raw_description,
                raw_image_url,
                category_id,
                now,
                norm_title,
                norm_url
            ],
        ) {
            Ok(_) => {
                let post_id = conn.last_insert_rowid();
                drop(conn);
                self.record_parsed(source_url, raw_title)?;
                Ok(Some(post_id))
            }
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_posts_by_status(&self, status: &str, limit: i64) -> Result<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             WHERE p.status = ?1
             ORDER BY p.created_at ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![status, limit], Self::map_post_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn count_posts_by_status(&self, status: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status = ?1",
            params![status],
            |r| r.get(0),
        )?;
        Ok(count)
    }

    pub fn claim_post_for_ai(&self, id: i64) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET status='processing' WHERE id=?1 AND status='new'",
            params![id],
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    pub fn release_post_ai_claim(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET status='new' WHERE id=?1 AND status='processing'",
            params![id],
        )?;
        Ok(())
    }

    pub fn record_ai_duplicate(
        &self,
        duplicate_url: &str,
        duplicate_title: &str,
        duplicate_description: &str,
        kept_post_id: Option<i64>,
        kept_title: Option<&str>,
        analysis: &DuplicateAiAnalysis,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO duplicate_records (
                duplicate_url, duplicate_title, duplicate_description,
                kept_post_id, kept_title, reason, created_at,
                ai_is_duplicate, ai_confidence, ai_explanation, ai_checked_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'ai_duplicate', ?6, ?7, ?8, ?9, ?6)",
            params![
                duplicate_url,
                duplicate_title,
                duplicate_description,
                kept_post_id,
                kept_title,
                now,
                analysis.is_duplicate as i32,
                analysis.confidence as i64,
                analysis.explanation,
            ],
        )?;
        Ok(())
    }

    pub fn reset_all_data(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM publish_log", [])?;
        conn.execute("DELETE FROM posts", [])?;
        conn.execute("DELETE FROM parsed_items", [])?;
        conn.execute("DELETE FROM duplicate_records", [])?;
        conn.execute("DELETE FROM app_meta WHERE key = 'last_fetch_at'", [])?;
        conn.execute("UPDATE sources SET last_fetched_at = NULL", [])?;
        Ok(())
    }

    pub fn get_duplicates_overview(&self) -> Result<DuplicatesOverview> {
        let kept_posts = self.get_posts(None)?;
        let kept_count = kept_posts.len() as i64;
        let duplicates = self.get_duplicate_records(200)?;
        let duplicates_count = self.count_duplicate_records()?;
        let groups = self.build_duplicate_groups(&duplicates)?;
        let kept_ids: HashSet<i64> = groups.iter().filter_map(|g| g.kept_post_id).collect();
        let standalone_posts = kept_posts
            .into_iter()
            .filter(|p| !kept_ids.contains(&p.id))
            .collect();

        Ok(DuplicatesOverview {
            kept_count,
            duplicates_count,
            groups,
            standalone_posts,
            ai_duplicate_check_enabled: false,
        })
    }

    pub fn get_recent_posts(&self, limit: i64) -> Result<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             ORDER BY p.created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], Self::map_post_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_duplicate_record(&self, id: i64) -> Result<DuplicateRecord> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, duplicate_url, duplicate_title, duplicate_description, kept_post_id,
                    kept_title, reason, created_at, ai_is_duplicate, ai_confidence,
                    ai_explanation, ai_checked_at
             FROM duplicate_records WHERE id = ?1",
            params![id],
            Self::map_duplicate_row,
        )
        .context("Duplicate record not found")
    }

    pub fn update_duplicate_ai(&self, id: i64, analysis: &DuplicateAiAnalysis) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE duplicate_records
             SET ai_is_duplicate = ?1, ai_confidence = ?2, ai_explanation = ?3, ai_checked_at = ?4
             WHERE id = ?5",
            params![
                analysis.is_duplicate as i32,
                analysis.confidence as i64,
                analysis.explanation,
                now,
                id
            ],
        )?;
        Ok(())
    }

    fn build_duplicate_groups(&self, duplicates: &[DuplicateRecord]) -> Result<Vec<DuplicateGroup>> {
        let mut by_kept: BTreeMap<Option<i64>, Vec<DuplicateRecord>> = BTreeMap::new();
        for duplicate in duplicates {
            by_kept
                .entry(duplicate.kept_post_id)
                .or_default()
                .push(duplicate.clone());
        }

        let mut groups = Vec::new();
        for (kept_post_id, group_duplicates) in by_kept {
            let kept_post = kept_post_id
                .and_then(|id| self.get_post(id).ok());
            groups.push(DuplicateGroup {
                kept_post_id,
                kept_post,
                duplicates: group_duplicates,
            });
        }
        Ok(groups)
    }

    fn get_duplicate_records(&self, limit: i64) -> Result<Vec<DuplicateRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, duplicate_url, duplicate_title, duplicate_description, kept_post_id,
                    kept_title, reason, created_at, ai_is_duplicate, ai_confidence,
                    ai_explanation, ai_checked_at
             FROM duplicate_records
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], Self::map_duplicate_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn map_duplicate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DuplicateRecord> {
        let ai_is_duplicate: Option<i64> = row.get(8)?;
        Ok(DuplicateRecord {
            id: row.get(0)?,
            duplicate_url: row.get(1)?,
            duplicate_title: row.get(2)?,
            duplicate_description: row.get(3)?,
            kept_post_id: row.get(4)?,
            kept_title: row.get(5)?,
            reason: row.get(6)?,
            created_at: row.get(7)?,
            ai_is_duplicate: ai_is_duplicate.map(|v| v != 0),
            ai_confidence: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
            ai_explanation: row.get(10)?,
            ai_checked_at: row.get(11)?,
        })
    }

    fn count_duplicate_records(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM duplicate_records", [], |r| r.get(0))?;
        Ok(count)
    }

    fn map_post_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Post> {
        Ok(Post {
            id: row.get(0)?,
            source_url: row.get(1)?,
            raw_title: row.get(2)?,
            raw_description: row.get(3)?,
            raw_image_url: row.get(4)?,
            ai_title: row.get(5)?,
            ai_text: row.get(6)?,
            ai_hashtags: row.get(7)?,
            category_id: row.get(8)?,
            category_name: row.get(9)?,
            status: row.get(10)?,
            vk_post_id: row.get(11)?,
            telegram_message_id: row.get(12)?,
            created_at: row.get(13)?,
            published_at: row.get(14)?,
            error_message: row.get(15)?,
        })
    }

    pub fn get_posts(&self, status: Option<&str>) -> Result<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let sql = if status.is_some() {
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             WHERE p.status = ?1 ORDER BY p.created_at DESC"
        } else {
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             ORDER BY p.created_at DESC"
        };

        let mut stmt = conn.prepare(sql)?;
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(Post {
                id: row.get(0)?,
                source_url: row.get(1)?,
                raw_title: row.get(2)?,
                raw_description: row.get(3)?,
                raw_image_url: row.get(4)?,
                ai_title: row.get(5)?,
                ai_text: row.get(6)?,
                ai_hashtags: row.get(7)?,
                category_id: row.get(8)?,
                category_name: row.get(9)?,
                status: row.get(10)?,
                vk_post_id: row.get(11)?,
                telegram_message_id: row.get(12)?,
                created_at: row.get(13)?,
                published_at: row.get(14)?,
                error_message: row.get(15)?,
            })
        };

        if let Some(s) = status {
            let rows = stmt.query_map(params![s], map_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        } else {
            let rows = stmt.query_map([], map_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        }
    }

    pub fn get_post(&self, id: i64) -> Result<Post> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             WHERE p.id = ?1",
            params![id],
            |row| {
                Ok(Post {
                    id: row.get(0)?,
                    source_url: row.get(1)?,
                    raw_title: row.get(2)?,
                    raw_description: row.get(3)?,
                    raw_image_url: row.get(4)?,
                    ai_title: row.get(5)?,
                    ai_text: row.get(6)?,
                    ai_hashtags: row.get(7)?,
                    category_id: row.get(8)?,
                    category_name: row.get(9)?,
                    status: row.get(10)?,
                    vk_post_id: row.get(11)?,
                    telegram_message_id: row.get(12)?,
                    created_at: row.get(13)?,
                    published_at: row.get(14)?,
                    error_message: row.get(15)?,
                })
            },
        )
        .context("Post not found")
    }

    pub fn update_post(&self, post: &Post) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET ai_title=?1, ai_text=?2, ai_hashtags=?3, status=?4,
             vk_post_id=?5, telegram_message_id=?6, published_at=?7, error_message=?8
             WHERE id=?9",
            params![
                post.ai_title,
                post.ai_text,
                post.ai_hashtags,
                post.status,
                post.vk_post_id,
                post.telegram_message_id,
                post.published_at,
                post.error_message,
                post.id
            ],
        )?;
        Ok(())
    }

    pub fn update_post_ai(
        &self,
        id: i64,
        title: &str,
        text: &str,
        hashtags: &str,
        auto_approve: bool,
    ) -> Result<()> {
        let status = if auto_approve {
            "approved"
        } else {
            "ai_processed"
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET ai_title=?1, ai_text=?2, ai_hashtags=?3, status=?4 WHERE id=?5",
            params![title, text, hashtags, status, id],
        )?;
        Ok(())
    }

    pub fn approve_post(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET status='approved' WHERE id=?1 AND status IN ('new', 'ai_processed')",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete_post(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM publish_log WHERE post_id = ?1", params![id])?;
        conn.execute("DELETE FROM posts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_queue_posts(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM publish_log WHERE post_id IN (
                SELECT id FROM posts WHERE status IN ('new', 'processing', 'ai_processed', 'approved', 'failed')
            )",
            [],
        )?;
        let deleted = conn.execute(
            "DELETE FROM posts WHERE status IN ('new', 'processing', 'ai_processed', 'approved', 'failed')",
            [],
        )?;
        Ok(deleted as i64)
    }

    pub fn get_next_publishable_post(&self) -> Result<Option<Post>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             WHERE p.status IN ('ai_processed', 'approved')
             ORDER BY p.created_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok(Post {
                    id: row.get(0)?,
                    source_url: row.get(1)?,
                    raw_title: row.get(2)?,
                    raw_description: row.get(3)?,
                    raw_image_url: row.get(4)?,
                    ai_title: row.get(5)?,
                    ai_text: row.get(6)?,
                    ai_hashtags: row.get(7)?,
                    category_id: row.get(8)?,
                    category_name: row.get(9)?,
                    status: row.get(10)?,
                    vk_post_id: row.get(11)?,
                    telegram_message_id: row.get(12)?,
                    created_at: row.get(13)?,
                    published_at: row.get(14)?,
                    error_message: row.get(15)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_new_posts(&self) -> Result<Vec<Post>> {
        self.get_posts(Some("new"))
    }

    pub fn get_published_posts(&self) -> Result<Vec<Post>> {
        self.get_posts(Some("published"))
    }

    pub fn get_recent_published_posts(&self, limit: i64) -> Result<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.source_url, p.raw_title, p.raw_description, p.raw_image_url,
                    p.ai_title, p.ai_text, p.ai_hashtags, p.category_id, c.name,
                    p.status, p.vk_post_id, p.telegram_message_id, p.created_at,
                    p.published_at, p.error_message
             FROM posts p LEFT JOIN categories c ON p.category_id = c.id
             WHERE p.status = 'published'
             ORDER BY p.published_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(Post {
                id: row.get(0)?,
                source_url: row.get(1)?,
                raw_title: row.get(2)?,
                raw_description: row.get(3)?,
                raw_image_url: row.get(4)?,
                ai_title: row.get(5)?,
                ai_text: row.get(6)?,
                ai_hashtags: row.get(7)?,
                category_id: row.get(8)?,
                category_name: row.get(9)?,
                status: row.get(10)?,
                vk_post_id: row.get(11)?,
                telegram_message_id: row.get(12)?,
                created_at: row.get(13)?,
                published_at: row.get(14)?,
                error_message: row.get(15)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn add_publish_log(
        &self,
        post_id: i64,
        platform: &str,
        success: bool,
        response: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO publish_log (post_id, platform, success, response, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![post_id, platform, success as i32, response, now],
        )?;
        Ok(())
    }

    pub fn get_publish_history(&self) -> Result<Vec<PublishLog>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, post_id, platform, success, response, created_at FROM publish_log ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PublishLog {
                id: row.get(0)?,
                post_id: row.get(1)?,
                platform: row.get(2)?,
                success: row.get::<_, i32>(3)? != 0,
                response: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        let conn = self.conn.lock().unwrap();
        let today = Utc::now().format("%Y-%m-%d").to_string();

        let posts_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE created_at LIKE ?1 || '%'",
            params![today],
            |r| r.get(0),
        )?;

        let posts_pending: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status IN ('new', 'processing', 'ai_processed', 'approved')",
            [],
            |r| r.get(0),
        )?;

        let posts_published: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status = 'published'",
            [],
            |r| r.get(0),
        )?;

        let sources_active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sources WHERE enabled = 1",
            [],
            |r| r.get(0),
        )?;

        let last_fetch_at: Option<String> = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'last_fetch_at'",
                [],
                |r| r.get(0),
            )
            .optional()?;

        let duplicates_total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM duplicate_records",
            [],
            |r| r.get(0),
        )?;

        let posts_waiting_ai: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status = 'new'",
            [],
            |r| r.get(0),
        )?;

        let posts_processing_ai: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status = 'processing'",
            [],
            |r| r.get(0),
        )?;

        let posts_ai_processed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status = 'ai_processed'",
            [],
            |r| r.get(0),
        )?;

        let posts_approved: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE status = 'approved'",
            [],
            |r| r.get(0),
        )?;

        Ok(DashboardStats {
            posts_today,
            posts_pending,
            posts_published,
            sources_active,
            last_fetch_at,
            duplicates_total,
            posts_waiting_ai,
            posts_processing_ai,
            posts_ai_processed,
            posts_approved,
        })
    }

    pub fn set_last_fetch_at(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO app_meta (key, value) VALUES ('last_fetch_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![now],
        )?;
        Ok(())
    }
}
