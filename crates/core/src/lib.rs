//! Immutable backup operations shared by CLI and future desktop adapters.
mod archive;
mod engine;
pub mod error;
mod jobs;
mod model;
mod paths;
mod restore;
mod scan;
mod state;

use anyhow::Result;
pub use engine::install_cancel_handler;
pub use jobs::Changes;
pub use model::*;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

pub struct App {
    pub(crate) home: PathBuf,
    _lock: File,
    state: state::State,
}
impl App {
    pub fn open(home: &Path) -> Result<Self> {
        Self::open_mode(home, false)
    }
    pub fn open_read_only(home: &Path) -> Result<Self> {
        Self::open_mode(home, true)
    }
    fn open_mode(home: &Path, read_only: bool) -> Result<Self> {
        if !read_only {
            paths::private_dir(home)?;
        }
        let home = paths::existing(home)?;
        let lock = OpenOptions::new()
            .create(!read_only)
            .truncate(false)
            .read(true)
            .write(true)
            .open(home.join("app.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| error::Failure {
            exit: 5,
            reason: "busy",
            message: "Another Smart Backup operation is active".into(),
        })?;
        if !read_only {
            paths::private_dir(&home.join("jobs"))?;
        }
        let state = state::State::open(&home, read_only)?;
        Ok(Self {
            home,
            _lock: lock,
            state,
        })
    }
    pub fn edit_job(&self, selector: &str, changes: Changes) -> Result<Job> {
        jobs::edit(&self.home, selector, changes)
    }
    pub fn delete_job(&self, selector: &str) -> Result<Job> {
        jobs::delete(&self.home, selector)
    }
    pub fn preview(&self, selector: &str) -> Result<serde_json::Value> {
        let job = self.job(selector)?;
        jobs::validate(&job, &self.home, true)?;
        let engine = engine::Engine::load(&self.home)?;
        let scan = scan::scan(&job)?;
        let mut value = serde_json::to_value(&scan)?;
        value["dry_run"] = true.into();
        value["output"] = serde_json::to_value(&job.output)?;
        value["estimated_temporary_bytes"] = scan
            .bytes
            .saturating_mul(2)
            .saturating_add(16 * 1024 * 1024)
            .into();
        value["available_output_bytes"] = fs2::available_space(&job.output)?.into();
        value["verification"] = "full".into();
        value["engine"] = serde_json::to_value(engine.config)?;
        value["argv_preview"] = serde_json::json!([
            "a",
            "-t7z",
            "-mx=5",
            "-snl",
            "-bd",
            "-y",
            "--",
            "<private-staging>/<unique-run>.7z",
            "."
        ]);
        Ok(value)
    }
    pub fn validate_config(&self) -> Result<serde_json::Value> {
        let (jobs, invalid) = jobs::list(&self.home)?;
        if !invalid.is_empty() {
            return error::config(
                "invalid_config",
                format!("Unreadable Job files: {}", invalid.join(", ")),
            );
        }
        for job in &jobs {
            jobs::validate(job, &self.home, true)?;
        }
        engine::Engine::load(&self.home)?;
        Ok(serde_json::json!({"valid":true,"jobs":jobs.len()}))
    }
    pub fn configure_engine(&self, path: &Path) -> Result<EngineConfig> {
        engine::Engine::configure(&self.home, path)
    }
    pub fn run(&self, selector: &str, force: bool) -> Result<Run> {
        let job = self.job(selector)?;
        self.state.interrupt_abandoned()?;
        let mut run = archive::new_run(&job);
        self.state.save(&run)?;
        state::event(&self.home, &run, "preflight")?;
        let result = (|| {
            jobs::validate(&job, &self.home, true)?;
            if !job.enabled {
                return error::config("job_disabled", "Enable the Job before running");
            }
            let engine = engine::Engine::load(&self.home)?;
            archive::recover(&engine, &job, &self.state)?;
            state::event(&self.home, &run, "scanning")?;
            let scan = scan::scan(&job)?;
            if scan.available_sources == 0
                || (!force
                    && scan.warnings.is_empty()
                    && self.state.baseline(&job.id)?.as_deref() == Some(&scan.fingerprint))
            {
                run.status = "skipped".into();
                run.reason_code = if scan.available_sources == 0 {
                    "no_available_sources"
                } else {
                    "no_changes"
                }
                .into();
                run.finished_at = Some(chrono::Utc::now().to_rfc3339());
                return Ok(run.clone());
            }
            state::event(&self.home, &run, "archiving")?;
            archive::snapshot(&engine, &job, &scan, run.clone())
        })();
        match result {
            Ok(completed) => {
                self.state.save(&completed)?;
                state::event(&self.home, &completed, "finished")?;
                archive::finish_commit(&job, &completed)?;
                Ok(completed)
            }
            Err(e) => {
                run.status = if e
                    .downcast_ref::<error::Failure>()
                    .is_some_and(|e| e.exit == 3)
                {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                run.reason_code = e
                    .downcast_ref::<error::Failure>()
                    .map(|e| e.reason)
                    .unwrap_or("run_failed")
                    .into();
                run.finished_at = Some(chrono::Utc::now().to_rfc3339());
                self.state.save(&run)?;
                state::event(&self.home, &run, "finished")?;
                Err(e)
            }
        }
    }
    pub fn history(&self, job: Option<&str>) -> Result<serde_json::Value> {
        let id = job.map(|j| self.job(j).map(|j| j.id)).transpose()?;
        Ok(serde_json::json!({"runs":self.state.list(id.as_deref())?}))
    }
    pub fn history_show(&self, id: &str) -> Result<Run> {
        self.state.get(id)?.ok_or_else(|| {
            error::Failure {
                exit: 4,
                reason: "run_not_found",
                message: "Run not found".into(),
            }
            .into()
        })
    }
    pub fn archive_list(&self, path: &Path, query: Option<&str>) -> Result<serde_json::Value> {
        let mut entries = restore::list(&engine::Engine::load(&self.home)?, path)?;
        if let Some(query) = query {
            entries.retain(|e| e.path.contains(query));
        }
        Ok(serde_json::json!({"entries":entries}))
    }
    pub fn restore(&self, path: &Path, output: &Path) -> Result<serde_json::Value> {
        restore::restore(&engine::Engine::load(&self.home)?, path, output)
    }
    pub fn verify(&self, path: &Path) -> Result<serde_json::Value> {
        archive::verify(&engine::Engine::load(&self.home)?, path)
    }
    pub fn create_job(
        &self,
        name: &str,
        sources: &[String],
        output: &Path,
        rules: &[String],
        follow_gitignore: bool,
    ) -> Result<Job> {
        jobs::create(&self.home, name, sources, output, rules, follow_gitignore)
    }
    pub fn job(&self, selector: &str) -> Result<Job> {
        jobs::get(&self.home, selector)
    }
    pub fn jobs(&self) -> Result<serde_json::Value> {
        let (jobs, invalid) = jobs::list(&self.home)?;
        Ok(serde_json::json!({"jobs": jobs, "invalid_configs": invalid}))
    }
}
