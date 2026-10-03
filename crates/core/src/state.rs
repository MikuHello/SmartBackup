use crate::{
    error::{config, fail},
    model::Run,
    paths,
};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use uuid::Uuid;

pub struct State {
    db: Connection,
}
impl State {
    pub fn open(home: &Path, read_only: bool) -> Result<Self> {
        if !read_only {
            paths::private_dir(&home.join("logs"))?;
        }
        let path = home.join("state.db");
        let mut db = if read_only {
            Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?
        } else {
            Connection::open(&path)?
        };
        let check = db.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0));
        let corrupt = match check {
            Ok(value) => value != "ok",
            Err(rusqlite::Error::SqliteFailure(code, _))
                if matches!(
                    code.code,
                    rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
                ) =>
            {
                true
            }
            Err(error) => return Err(error.into()),
        };
        if corrupt {
            if read_only {
                return fail(
                    5,
                    "database_corrupt",
                    "History database is damaged; Dry Run will not rebuild it. A normal operation can preserve and rebuild it.",
                );
            }
            drop(db);
            let suffix = Uuid::new_v4();
            fs::rename(&path, home.join(format!("state.corrupt-{suffix}.db")))?;
            for side in ["-journal", "-wal", "-shm"] {
                let path = home.join(format!("state.db{side}"));
                if path.exists() {
                    fs::rename(path, home.join(format!("state.corrupt-{suffix}.db{side}")))?;
                }
            }
            db = Connection::open(&path)?;
        }
        let version: i32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return config(
                "unsupported_schema",
                "Database was created by a newer application",
            );
        }
        if !read_only && version == 0 {
            db.execute_batch("PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, job_id TEXT NOT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL, fingerprint TEXT, payload TEXT NOT NULL); CREATE INDEX IF NOT EXISTS runs_job ON runs(job_id,started_at); PRAGMA user_version=1;")?;
        }
        if !read_only {
            db.execute_batch(
                "CREATE INDEX IF NOT EXISTS runs_running ON runs(status) WHERE status='running';",
            )?;
        }
        Ok(Self { db })
    }
    pub fn interrupt_abandoned(&self) -> Result<()> {
        // Startup needs only unfinished Runs, not sorted/deserialized complete history.
        let mut statement = self
            .db
            .prepare("SELECT payload FROM runs WHERE status='running'")?;
        let rows = statement.query_map([], |r| r.get::<_, String>(0))?;
        let runs: Result<Vec<Run>> = rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect();
        let runs = runs?;
        drop(statement);
        for mut run in runs {
            run.status = "interrupted".into();
            run.reason_code = "process_interrupted".into();
            run.finished_at = Some(Utc::now().to_rfc3339());
            self.save(&run)?;
        }
        Ok(())
    }
    pub fn save(&self, run: &Run) -> Result<()> {
        self.db.execute("INSERT INTO runs(id,job_id,status,started_at,fingerprint,payload) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET status=excluded.status,fingerprint=excluded.fingerprint,payload=excluded.payload",params![run.id,run.job_id,run.status,run.started_at,run.fingerprint,serde_json::to_string(run)?])?;
        Ok(())
    }
    pub fn list(&self, job: Option<&str>) -> Result<Vec<Run>> {
        let mut statement=self.db.prepare("SELECT payload FROM runs WHERE (?1 IS NULL OR job_id=?1) ORDER BY started_at DESC,id DESC")?;
        let rows = statement.query_map(params![job], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn get(&self, id: &str) -> Result<Option<Run>> {
        let payload: Option<String> = self
            .db
            .query_row("SELECT payload FROM runs WHERE id=?1", params![id], |r| {
                r.get(0)
            })
            .optional()?;
        payload
            .map(|p| serde_json::from_str(&p).map_err(Into::into))
            .transpose()
    }
    pub fn baseline(&self, job: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT fingerprint FROM runs WHERE job_id=?1 AND status='success' AND fingerprint IS NOT NULL ORDER BY started_at DESC LIMIT 1",params![job],|r|r.get(0)).optional()?)
    }
}
pub fn event(home: &Path, run: &Run, phase: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("logs").join(format!("{}.jsonl", run.id)))?;
    writeln!(
        file,
        "{}",
        serde_json::json!({"schema_version":1,"at":Utc::now().to_rfc3339(),"run_id":run.id,"phase":phase,"status":run.status,"reason_code":run.reason_code})
    )?;
    file.sync_all()?;
    Ok(())
}
