//! Persistența proiectelor în SQLite.
//!
//! Aceeași schemă ca aplicația Tauri (`projects`: id, name, locality, created_at,
//! updated_at, data = JSON-ul complet al proiectului). La prima pornire, dacă baza
//! nouă e goală, proiectele din baza aplicației Tauri (`ro.hidro.calc/hidro.db`)
//! sunt COPIATE aici — baza veche nu e modificată.

use hidro_core::model::ProjectData;
use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};

const CREATE_TABLE: &str = "
  CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL DEFAULT '',
    locality    TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    data        TEXT NOT NULL
  )";

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub locality: String,
    pub updated_at: String,
}

pub struct Db {
    conn: Connection,
}

fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("ro", "hidro", "HidroCalc")
}

/// Directorul de date al aplicației (ex. macOS: `~/Library/Application Support/ro.hidro.HidroCalc`).
pub fn data_dir() -> Option<PathBuf> {
    project_dirs().map(|d| d.data_dir().to_path_buf())
}

/// Un subdirector din cache-ul aplicației (dale OSM, dale de altitudini).
pub fn cache_dir(sub: &str) -> Option<PathBuf> {
    project_dirs().map(|d| d.cache_dir().join(sub))
}

/// Cache-ul dalelor de altitudini (Terrarium), pe nivelul de zoom folosit.
pub fn dem_cache_dir() -> Option<PathBuf> {
    cache_dir("terrarium").map(|d| d.join(hidro_core::elevation::ZOOM.to_string()))
}

/// Baza de date a aplicației Tauri (`app_config_dir()/hidro.db`, identifier `ro.hidro.calc`).
pub fn legacy_db_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|b| b.config_dir().join("ro.hidro.calc").join("hidro.db"))
}

impl Db {
    pub fn open_default() -> Result<Self, String> {
        let dir = data_dir().ok_or("Directorul de date nu e disponibil")?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let db = Self::open(&dir.join("hidro.db"))?;
        if db.count()? == 0 {
            if let Some(legacy) = legacy_db_path().filter(|p| p.exists()) {
                match db.import_legacy(&legacy) {
                    Ok(n) if n > 0 => log::info!("{n} proiecte copiate din {}", legacy.display()),
                    Ok(_) => {}
                    Err(e) => log::warn!("Importul din baza veche a eșuat: {e}"),
                }
            }
        }
        Ok(db)
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute(CREATE_TABLE, []).map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    fn count(&self) -> Result<i64, String> {
        self.conn.query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0)).map_err(|e| e.to_string())
    }

    /// Copiază rândurile din baza aplicației Tauri (deschisă doar pentru citire).
    pub fn import_legacy(&self, legacy: &Path) -> Result<usize, String> {
        let old = Connection::open_with_flags(legacy, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
        let mut stmt = old.prepare("SELECT id, name, locality, created_at, updated_at, data FROM projects").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?)))
            .map_err(|e| e.to_string())?;
        let mut n = 0;
        for row in rows {
            let (id, name, locality, created, updated, data) = row.map_err(|e| e.to_string())?;
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO projects (id, name, locality, created_at, updated_at, data)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, name, locality, created, updated, data],
                )
                .map_err(|e| e.to_string())?;
            n += 1;
        }
        Ok(n)
    }

    pub fn list(&self) -> Result<Vec<IndexEntry>, String> {
        let mut stmt = self.conn.prepare("SELECT id, name, locality, updated_at FROM projects ORDER BY updated_at DESC").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok(IndexEntry { id: r.get(0)?, name: r.get(1)?, locality: r.get(2)?, updated_at: r.get(3)? })).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    pub fn load(&self, id: &str) -> Result<Option<ProjectData>, String> {
        let data: Option<String> = self
            .conn
            .query_row("SELECT data FROM projects WHERE id = ?1", [id], |r| r.get(0))
            .map(Some)
            .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })
            .map_err(|e| e.to_string())?;
        data.map(|d| serde_json::from_str(&d).map_err(|e| format!("Date corupte pentru proiectul {id}: {e}"))).transpose()
    }

    pub fn save(&self, p: &ProjectData) -> Result<(), String> {
        let json = serde_json::to_string(p).map_err(|e| e.to_string())?;
        let name = if p.general.name.is_empty() { "Proiect fără nume" } else { &p.general.name };
        let locality = if p.general.locality.is_empty() { "-" } else { &p.general.locality };
        self.conn
            .execute(
                "INSERT OR REPLACE INTO projects (id, name, locality, created_at, updated_at, data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![p.id, name, locality, p.created_at, p.updated_at, json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Face scrierile să eșueze (pentru testele de eroare la salvare).
    #[cfg(test)]
    pub fn break_for_test(&self) {
        self.conn.execute("DROP TABLE projects", []).unwrap();
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM projects WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_list_delete() {
        let dir = std::env::temp_dir().join(format!("hidrocalc-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.db")).unwrap();
        let mut p = ProjectData::new("p1".into(), "2026-01-01T00:00:00.000Z".into(), "2026-01-01".into());
        p.general.name = "Sat".into();
        db.save(&p).unwrap();
        assert_eq!(db.list().unwrap()[0].name, "Sat");
        assert_eq!(db.load("p1").unwrap().unwrap(), p);
        db.delete("p1").unwrap();
        assert!(db.load("p1").unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Verifică pe baza reală a aplicației Tauri (dacă există) că toate proiectele
    /// se citesc și se rescriu fără pierderi: `cargo test -- --ignored legacy`.
    #[test]
    #[ignore]
    fn legacy_projects_roundtrip_losslessly() {
        let Some(path) = legacy_db_path().filter(|p| p.exists()) else { return };
        let old = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut stmt = old.prepare("SELECT id, data FROM projects").unwrap();
        let rows: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect();
        for (id, data) in &rows {
            let orig: serde_json::Value = serde_json::from_str(data).unwrap();
            let p: ProjectData = serde_json::from_value(orig.clone()).unwrap_or_else(|e| panic!("{id}: {e}"));
            let back = serde_json::to_value(&p).unwrap();
            assert!(json_eq(&orig, &back), "proiectul {id} nu se păstrează identic");
        }
        println!("{} proiecte verificate", rows.len());
    }

    /// `a` (originalul) e conținut în `b`: orice cheie din original există în rezultat
    /// cu aceeași valoare (1 == 1.0). Cheile noi din `b` (valori implicite) sunt permise.
    fn json_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        use serde_json::Value::*;
        match (a, b) {
            (Number(x), Number(y)) => x.as_f64() == y.as_f64(),
            (Object(x), Object(y)) => x.iter().all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w))),
            (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| json_eq(v, w)),
            _ => a == b,
        }
    }
}
