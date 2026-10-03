use crate::{
    engine::{self, Engine},
    error::{config, fail},
    jobs,
    model::{Job, Manifest, Run},
    paths,
    scan::{self, Scan},
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitJournal {
    pub schema_version: u32,
    pub run: Run,
    pub archive_name: String,
    pub archive_hash: String,
    pub checksum_hash: String,
}

pub fn verify(engine: &Engine, archive: &Path) -> Result<serde_json::Value> {
    let archive = paths::existing(archive)?;
    engine.validate()?;
    engine.run(
        &[
            "t".into(),
            "-bd".into(),
            "--".into(),
            archive.as_os_str().into(),
        ],
        None,
    )?;
    let actual = engine::hash(&archive)?;
    let checksum = PathBuf::from(format!("{}.sha256", archive.display()));
    let expected = fs::read_to_string(&checksum).map_err(|_| crate::error::Failure {
        exit: 6,
        reason: "checksum_missing",
        message: "Full verification requires a matching .sha256 file".into(),
    })?;
    let expected_line = format!(
        "{actual}  {}\n",
        archive
            .file_name()
            .context("Archive filename")?
            .to_string_lossy()
    );
    if expected != expected_line {
        return fail(
            6,
            "checksum_mismatch",
            "Archive SHA-256 or checksum filename does not match",
        );
    }
    Ok(serde_json::json!({"archive":archive,"sha256":actual,"verification_state":"verified"}))
}
fn copy_file(entry: &scan::Entry, destination: &Path) -> Result<()> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut input = options.open(&entry.source)?;
    let before = input.metadata()?;
    if !before.is_file()
        || before.len() != entry.bytes
        || before.modified()?.duration_since(UNIX_EPOCH)?.as_nanos() != entry.modified_ns
    {
        return fail(
            6,
            "source_changed",
            "Source changed between scanning and reading",
        );
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut buffer = [0u8; 65536];
    let mut size = 0;
    loop {
        engine::check_cancelled()?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
        size += n as u64;
    }
    let after = input.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? || size != entry.bytes
    {
        return fail(
            6,
            "source_changed",
            "Source changed while being copied; no Artifact committed",
        );
    }
    output.sync_all()?;
    filetime::set_file_mtime(
        destination,
        filetime::FileTime::from_last_modification_time(&before),
    )?;
    Ok(())
}
fn capture(scan: &Scan, root: &Path) -> Result<()> {
    for entry in &scan.entries {
        engine::check_cancelled()?;
        let destination = root.join(paths::safe_entry(&entry.path)?);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        match entry.kind.as_str() {
            "directory" => fs::create_dir_all(&destination)?,
            "file" => copy_file(entry, &destination)?,
            "symlink" => {
                #[cfg(unix)]
                std::os::unix::fs::symlink(
                    entry.link_target.as_ref().context("Link target missing")?,
                    &destination,
                )?;
                #[cfg(not(unix))]
                return config(
                    "unsupported_in_version",
                    "Symlink capture is not implemented on this platform",
                );
            }
            _ => return config("unsupported_file_type", "Unknown entry type"),
        }
    }
    for entry in scan.entries.iter().rev().filter(|e| e.kind == "directory") {
        let metadata = fs::symlink_metadata(&entry.source)?;
        filetime::set_file_mtime(
            root.join(&entry.path),
            filetime::FileTime::from_last_modification_time(&metadata),
        )?;
    }
    Ok(())
}
pub fn new_run(job: &Job) -> Run {
    Run {
        schema_version: 1,
        id: Uuid::new_v4().to_string(),
        job_id: job.id.clone(),
        job_name: job.name.clone(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        status: "running".into(),
        reason_code: "archiving".into(),
        artifact: None,
        sha256: None,
        verification_state: "unverified".into(),
        fingerprint: None,
        warnings: vec![],
    }
}
pub fn snapshot(engine: &Engine, job: &Job, scan: &Scan, mut run: Run) -> Result<Run> {
    engine.validate()?;
    let free = fs2::available_space(&job.output)?;
    let needed = scan
        .bytes
        .saturating_mul(2)
        .saturating_add(16 * 1024 * 1024);
    if free < needed {
        return fail(
            5,
            "insufficient_space",
            format!("Capture and archive need approximately {needed} bytes, {free} available"),
        );
    }
    let staging = job.output.join(".smart-backup-staging");
    paths::private_dir(&staging)?;
    let temp = tempfile::Builder::new()
        .prefix(&format!("{}-", run.id))
        .tempdir_in(&staging)?;
    let capture_root = temp.path().join("capture");
    fs::create_dir(&capture_root)?;
    capture(scan, &capture_root)?;
    let after = scan::scan(job)?;
    if after.fingerprint != scan.fingerprint {
        return fail(
            6,
            "source_changed",
            "Source set changed during capture; retry with quiescent files",
        );
    }
    let manifest_dir = capture_root.join(".smart-backup");
    fs::create_dir(&manifest_dir)?;
    let manifest = Manifest {
        schema_version: 1,
        job_id: job.id.clone(),
        run_id: run.id.clone(),
        archive_id: run.id.clone(),
        job_name: job.name.clone(),
        application_version: env!("CARGO_PKG_VERSION").into(),
        engine_version: engine.config.version.clone(),
        created_at: run.started_at.clone(),
        format: "7z".into(),
        source_aliases: job.sources.iter().map(|s| s.alias.clone()).collect(),
        files: scan.files,
        directories: scan.directories,
        bytes: scan.bytes,
        verification: "full".into(),
        rules_hash: scan.rules_hash.clone(),
    };
    fs::write(
        manifest_dir.join("manifest.toml"),
        toml::to_string_pretty(&manifest)?,
    )?;
    let archive_name = format!(
        "{}_{}_{}.7z",
        job.id,
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        run.id
    );
    let partial = temp.path().join(&archive_name);
    engine.run(
        &[
            "a".into(),
            "-t7z".into(),
            "-mx=5".into(),
            "-snl".into(),
            "-bd".into(),
            "-y".into(),
            "--".into(),
            partial.as_os_str().into(),
            ".".into(),
        ],
        Some(&capture_root),
    )?;
    engine.run(
        &[
            "t".into(),
            "-bd".into(),
            "--".into(),
            partial.as_os_str().into(),
        ],
        None,
    )?;
    // Windows requires write access to flush the newly created staging archive.
    OpenOptions::new().write(true).open(&partial)?.sync_all()?;
    let hash = engine::hash(&partial)?;
    let sidecar = temp.path().join(format!("{archive_name}.sha256"));
    jobs::write_new(&sidecar, format!("{hash}  {archive_name}\n").as_bytes())?;
    run.status = if scan.warnings.is_empty() {
        "success"
    } else {
        "warning"
    }
    .into();
    run.reason_code = if scan.warnings.is_empty() {
        "completed"
    } else {
        "source_warnings"
    }
    .into();
    run.warnings = scan.warnings.clone();
    run.finished_at = Some(Utc::now().to_rfc3339());
    run.artifact = Some(job.output.join(&archive_name));
    run.sha256 = Some(hash.clone());
    run.verification_state = "verified".into();
    run.fingerprint = Some(scan.fingerprint.clone());
    let journal = CommitJournal {
        schema_version: 1,
        run: run.clone(),
        archive_name: archive_name.clone(),
        archive_hash: hash,
        checksum_hash: engine::hash(&sidecar)?,
    };
    jobs::write_new(
        &temp.path().join("commit.json"),
        &serde_json::to_vec_pretty(&journal)?,
    )?;
    engine::check_cancelled()?;
    // Preserve journal after this point: a failed commit is recovered on the next Run.
    let _finalizing = engine::Finalizing::begin();
    let temp_path = temp.keep();
    commit_file(
        &partial,
        &job.output.join(&archive_name),
        &journal.archive_hash,
    )?;
    commit_file(
        &sidecar,
        &job.output.join(format!("{archive_name}.sha256")),
        &journal.checksum_hash,
    )?;
    paths::sync_dir(&job.output)?;
    // Journal cleanup occurs after the caller persists the delivered Run.
    let _ = temp_path;
    Ok(run)
}
fn commit_file(source: &Path, destination: &Path, expected: &str) -> Result<()> {
    match fs::hard_link(source, destination) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return fail(
                6,
                "artifact_name_race",
                "Artifact name already exists; existing bytes preserved",
            );
        }
        Err(_) => {
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            let mut input = File::open(source)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
        }
    }
    // Cancellation is ignored during finalization, but the digest still must match.
    if engine::hash(destination)? != expected {
        return fail(
            6,
            "commit_hash_mismatch",
            "Committed file checksum mismatch; journal retained",
        );
    }
    Ok(())
}

fn journals(job: &Job) -> Result<Vec<(PathBuf, CommitJournal)>> {
    let root = job.output.join(".smart-backup-staging");
    if !root.exists() {
        return Ok(vec![]);
    }
    if fs::symlink_metadata(&root)?.file_type().is_symlink() {
        return fail(5, "unsafe_staging", "Staging directory is a symlink");
    }
    let mut result = vec![];
    for dir in fs::read_dir(root)? {
        let dir = dir?;
        if !dir.file_type()?.is_dir() {
            continue;
        }
        let path = dir.path().join("commit.json");
        if !path.is_file() || fs::symlink_metadata(&path)?.file_type().is_symlink() {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(journal) = serde_json::from_slice::<CommitJournal>(&bytes) else {
            continue;
        };
        if journal.schema_version != 1
            || journal.run.job_id != job.id
            || Uuid::parse_str(&journal.run.id).is_err()
        {
            continue;
        }
        if !dir
            .file_name()
            .to_string_lossy()
            .starts_with(&format!("{}-", journal.run.id))
        {
            continue;
        }
        if paths::safe_component(&journal.archive_name).is_err()
            || journal.run.artifact.as_ref() != Some(&job.output.join(&journal.archive_name))
        {
            continue;
        }
        result.push((dir.path(), journal));
    }
    Ok(result)
}
pub fn finish_commit(job: &Job, run: &Run) -> Result<()> {
    for (path, journal) in journals(job)? {
        if journal.run.id == run.id && journal.archive_hash == run.sha256.as_deref().unwrap_or("") {
            fs::remove_dir_all(path)?;
        }
    }
    Ok(())
}
pub fn recover(engine: &Engine, job: &Job, state: &crate::state::State) -> Result<()> {
    for (path, journal) in journals(job)? {
        let Some(record) = state.get(&journal.run.id)? else {
            continue;
        };
        if record.job_id != job.id {
            continue;
        }
        let archive = job.output.join(&journal.archive_name);
        let checksum = job.output.join(format!("{}.sha256", journal.archive_name));
        if archive.is_file()
            && checksum.is_file()
            && engine::hash(&archive)? == journal.archive_hash
            && engine::hash(&checksum)? == journal.checksum_hash
        {
            let text = engine.run(
                &[
                    "x".into(),
                    "-so".into(),
                    "-spd".into(),
                    "--".into(),
                    archive.as_os_str().into(),
                    ".smart-backup/manifest.toml".into(),
                ],
                None,
            )?;
            let manifest: Manifest = toml::from_str(&text)?;
            if manifest.run_id != journal.run.id || manifest.job_id != job.id {
                continue;
            }
            state.save(&journal.run)?;
            fs::remove_dir_all(path)?;
        }
        // Incomplete/unknown groups remain quarantined. Never delete user files on inference.
    }
    Ok(())
}
