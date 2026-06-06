use crate::models::{Category, DashboardStats, Post, PublishLog, Source};
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
            "#,
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
    ) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE source_url = ?1",
            params![source_url],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Ok(None);
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO posts (source_url, raw_title, raw_description, raw_image_url, category_id, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'new', ?6)",
            params![source_url, raw_title, raw_description, raw_image_url, category_id, now],
        )?;
        Ok(Some(conn.last_insert_rowid()))
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
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET ai_title=?1, ai_text=?2, ai_hashtags=?3, status='ai_processed' WHERE id=?4",
            params![title, text, hashtags, id],
        )?;
        Ok(())
    }

    pub fn delete_post(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM posts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_new_posts(&self) -> Result<Vec<Post>> {
        self.get_posts(Some("new"))
    }

    pub fn get_published_posts(&self) -> Result<Vec<Post>> {
        self.get_posts(Some("published"))
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
            "SELECT COUNT(*) FROM posts WHERE status IN ('new', 'ai_processed', 'approved')",
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

        Ok(DashboardStats {
            posts_today,
            posts_pending,
            posts_published,
            sources_active,
            last_fetch_at,
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
