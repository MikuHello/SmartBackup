use crate::{
    error::{config, fail},
    jobs,
    model::{EngineConfig, GlobalConfig},
    paths,
};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{Read, Seek},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub static CANCELLED: AtomicBool = AtomicBool::new(false);
static FINALIZING: AtomicBool = AtomicBool::new(false);
pub struct Finalizing;
impl Finalizing {
    pub fn begin() -> Self {
        FINALIZING.store(true, Ordering::SeqCst);
        Self
    }
}
impl Drop for Finalizing {
    fn drop(&mut self) {
        FINALIZING.store(false, Ordering::SeqCst);
    }
}
pub fn install_cancel_handler() -> Result<()> {
    ctrlc::set_handler(|| {
        CANCELLED.store(true, Ordering::SeqCst);
    })?;
    Ok(())
}
pub fn check_cancelled() -> Result<()> {
    if CANCELLED.load(Ordering::Relaxed) && !FINALIZING.load(Ordering::SeqCst) {
        return fail(3, "cancelled", "Operation cancelled");
    }
    Ok(())
}
pub fn hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        check_cancelled()?;
        let len = file.read(&mut buf)?;
        if len == 0 {
            break;
        }
        digest.update(&buf[..len]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn resolve(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return paths::existing(path);
    }
    if let Some(search) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&search) {
            let candidate = dir.join(path);
            if candidate.is_file() {
                return paths::existing(&candidate);
            }
        }
    }
    fail(
        5,
        "engine_not_found",
        "7-Zip executable not found; specify an absolute path",
    )
}
#[derive(Clone)]
pub struct Engine {
    pub config: EngineConfig,
}
impl Engine {
    pub fn configure(home: &Path, path: &Path) -> Result<EngineConfig> {
        let path = resolve(path)?;
        let sha256 = hash(&path)?;
        let mut engine = Self {
            config: EngineConfig {
                path,
                sha256,
                version: String::new(),
                provenance: "user_provided_hash_pinned".into(),
            },
        };
        let output = engine.run(&["i".into()], None)?;
        if !output.contains("7-Zip") || !output.lines().any(|l| l.contains("7z") && l.contains('C'))
        {
            return config(
                "unsupported_engine",
                "Engine must provide 7z creation capability",
            );
        }
        engine.config.version = output
            .lines()
            .find(|l| l.contains("7-Zip"))
            .unwrap_or("unknown")
            .trim()
            .to_string();
        let global = GlobalConfig {
            schema_version: 1,
            engine: engine.config.clone(),
        };
        let path = home.join("config.toml");
        let bytes = toml::to_string_pretty(&global)?;
        if path.exists() {
            jobs::replace(&path, bytes.as_bytes())?;
        } else {
            jobs::write_new(&path, bytes.as_bytes())?;
        }
        Ok(engine.config)
    }
    pub fn load(home: &Path) -> Result<Self> {
        let content =
            fs::read_to_string(home.join("config.toml")).map_err(|_| crate::error::Failure {
                exit: 5,
                reason: "engine_not_configured",
                message: "Run: smart-backup engine configure --path /absolute/path/to/7zz".into(),
            })?;
        let global: GlobalConfig = toml::from_str(&content)?;
        if global.schema_version != 1 {
            return config("unsupported_schema", "Unknown global config schema");
        }
        let engine = Self {
            config: global.engine,
        };
        engine.validate()?;
        Ok(engine)
    }
    pub fn validate(&self) -> Result<()> {
        if hash(&self.config.path)? != self.config.sha256 {
            return fail(
                5,
                "engine_hash_mismatch",
                "7-Zip changed since configuration; inspect it before explicitly configuring again",
            );
        }
        Ok(())
    }
    pub fn run(&self, args: &[OsString], cwd: Option<&Path>) -> Result<String> {
        let mut log = tempfile::tempfile()?;
        self.spawn(args, cwd, log.try_clone()?, log.try_clone()?)?;
        log.rewind()?;
        let mut text = String::new();
        log.take(8 * 1024 * 1024 + 1).read_to_string(&mut text)?;
        if text.len() > 8 * 1024 * 1024 {
            return fail(
                6,
                "engine_output_too_large",
                "Engine output exceeds prototype limit; no truncated listing will be used",
            );
        }
        Ok(text)
    }
    pub fn to_file(&self, args: &[OsString], output: File) -> Result<()> {
        self.spawn(args, None, output, tempfile::tempfile()?)
    }
    fn spawn(
        &self,
        args: &[OsString],
        cwd: Option<&Path>,
        stdout: File,
        stderr: File,
    ) -> Result<()> {
        check_cancelled()?;
        let mut command = Command::new(&self.config.path);
        command
            .env("TZ", "UTC")
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr.try_clone()?);
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        let mut child = command.spawn().context("Starting 7-Zip")?;
        loop {
            if CANCELLED.load(Ordering::Relaxed) && !FINALIZING.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return fail(3, "cancelled", "7-Zip cancelled");
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return fail(
                        6,
                        "engine_failed",
                        format!(
                            "7-Zip returned {}; operation was not committed",
                            status.code().unwrap_or(-1)
                        ),
                    );
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
