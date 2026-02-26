/// SQLite-based cache for AST analysis, assembly, and graph layout results.

use rusqlite::{params, Connection};

pub struct CacheManager {
    conn: Option<Connection>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self { conn: None }
    }

    pub fn open(&mut self, db_path: &str) -> bool {
        match Connection::open(db_path) {
            Ok(conn) => {
                self.conn = Some(conn);
                self.create_tables();
                true
            }
            Err(e) => {
                eprintln!("Failed to open cache DB: {}", e);
                false
            }
        }
    }

    pub fn close(&mut self) {
        self.conn = None;
    }

    // ===== AST cache =====

    pub fn has_ast_cache(&self, file_path: &str, mtime: u64, flags_hash: u64) -> bool {
        let conn = match &self.conn {
            Some(c) => c,
            None => return false,
        };

        conn.query_row(
            "SELECT 1 FROM ast_cache WHERE file_path = ?1 AND mtime = ?2 AND flags_hash = ?3",
            params![file_path, mtime as i64, flags_hash as i64],
            |_| Ok(()),
        )
        .is_ok()
    }

    pub fn store_ast_cache(
        &self,
        file_path: &str,
        mtime: u64,
        flags_hash: u64,
        data: &[u8],
    ) {
        let conn = match &self.conn {
            Some(c) => c,
            None => return,
        };

        conn.execute(
            "INSERT OR REPLACE INTO ast_cache (file_path, mtime, flags_hash, data) VALUES (?1, ?2, ?3, ?4)",
            params![file_path, mtime as i64, flags_hash as i64, data],
        )
        .ok();
    }

    pub fn load_ast_cache(&self, file_path: &str) -> Option<Vec<u8>> {
        let conn = self.conn.as_ref()?;

        conn.query_row(
            "SELECT data FROM ast_cache WHERE file_path = ?1",
            params![file_path],
            |row| row.get(0),
        )
        .ok()
    }

    // ===== Assembly cache =====

    pub fn has_asm_cache(&self, obj_path: &str, mtime: u64) -> bool {
        let conn = match &self.conn {
            Some(c) => c,
            None => return false,
        };

        conn.query_row(
            "SELECT 1 FROM asm_cache WHERE obj_path = ?1 AND mtime = ?2",
            params![obj_path, mtime as i64],
            |_| Ok(()),
        )
        .is_ok()
    }

    pub fn store_asm_cache(&self, obj_path: &str, mtime: u64, data: &[u8]) {
        let conn = match &self.conn {
            Some(c) => c,
            None => return,
        };

        conn.execute(
            "INSERT OR REPLACE INTO asm_cache (obj_path, mtime, data) VALUES (?1, ?2, ?3)",
            params![obj_path, mtime as i64, data],
        )
        .ok();
    }

    pub fn load_asm_cache(&self, obj_path: &str) -> Option<Vec<u8>> {
        let conn = self.conn.as_ref()?;

        conn.query_row(
            "SELECT data FROM asm_cache WHERE obj_path = ?1",
            params![obj_path],
            |row| row.get(0),
        )
        .ok()
    }

    // ===== Layout cache =====

    pub fn store_layout(&self, graph_hash: &str, layout_json: &str) {
        let conn = match &self.conn {
            Some(c) => c,
            None => return,
        };

        conn.execute(
            "INSERT OR REPLACE INTO layout_cache (graph_hash, layout_json) VALUES (?1, ?2)",
            params![graph_hash, layout_json],
        )
        .ok();
    }

    pub fn load_layout(&self, graph_hash: &str) -> Option<String> {
        let conn = self.conn.as_ref()?;

        conn.query_row(
            "SELECT layout_json FROM layout_cache WHERE graph_hash = ?1",
            params![graph_hash],
            |row| row.get(0),
        )
        .ok()
    }

    // ===== Internal =====

    fn create_tables(&self) {
        let conn = match &self.conn {
            Some(c) => c,
            None => return,
        };

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS ast_cache (
                file_path   TEXT PRIMARY KEY,
                mtime       INTEGER,
                flags_hash  INTEGER,
                data        BLOB,
                updated_at  INTEGER DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS asm_cache (
                obj_path    TEXT PRIMARY KEY,
                mtime       INTEGER,
                data        BLOB,
                updated_at  INTEGER DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS layout_cache (
                graph_hash  TEXT PRIMARY KEY,
                layout_json TEXT,
                updated_at  INTEGER DEFAULT (strftime('%s','now'))
            );
            ",
        )
        .ok();
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CacheManager {
    fn drop(&mut self) {
        self.close();
    }
}
