use crate::{
    engine::{self, Engine},
    error::{config, fail},
    paths,
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, OpenOptions},
    path::Path,
};

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub bytes: u64,
    pub kind: String,
    pub modified: Option<String>,
}
pub fn list(engine: &Engine, archive: &Path) -> Result<Vec<ArchiveEntry>> {
    engine.validate()?;
    let archive = paths::existing(archive)?;
    let text = engine.run(
        &[
            "l".into(),
            "-slt".into(),
            "-ba".into(),
            "-sccUTF-8".into(),
            "--".into(),
            archive.as_os_str().into(),
        ],
        None,
    )?;
    let text = text.replace("\r\n", "\n");
    let mut result = vec![];
    let mut seen = HashSet::new();
    for block in text.trim().split("\n\n") {
        if block.trim().is_empty() {
            continue;
        }
        let mut fields = BTreeMap::new();
        for line in block.lines().filter(|l| !l.is_empty()) {
            let Some((key, value)) = line.split_once(" = ") else {
                return config(
                    "unsafe_archive_listing",
                    "Ambiguous archive listing (control characters or unsupported format)",
                );
            };
            if fields.insert(key, value).is_some() {
                return config(
                    "unsafe_archive_listing",
                    "Duplicate field in archive listing",
                );
            }
        }
        let path = fields
            .get("Path")
            .context("Archive listing missing Path")?
            .to_string();
        let normalized = paths::safe_entry(&path)?;
        let path = normalized
            .to_str()
            .context("Non-UTF8 archive path")?
            .replace('\\', "/");
        if !seen.insert(paths::key(&path)) {
            return config(
                "archive_path_collision",
                "Duplicate or case-folded archive path",
            );
        }
        let attr = fields.get("Attributes").copied().unwrap_or("");
        if fields.get("Encrypted") == Some(&"+") {
            return config(
                "unsupported_in_version",
                "Encrypted archive restore is unavailable in this prototype",
            );
        }
        let kind = if fields.contains_key("Hard Link") {
            "hardlink"
        } else if attr.split_whitespace().any(|s| s.starts_with('l'))
            || fields.contains_key("Symbolic Link")
        {
            "symlink"
        } else if attr.starts_with('D') || fields.get("Folder") == Some(&"+") {
            "directory"
        } else {
            "file"
        };
        result.push(ArchiveEntry {
            path,
            bytes: fields.get("Size").context("Entry size missing")?.parse()?,
            kind: kind.into(),
            modified: fields
                .get("Modified")
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string()),
        });
    }
    Ok(result)
}
pub fn restore(engine: &Engine, archive: &Path, output: &Path) -> Result<serde_json::Value> {
    if fs::symlink_metadata(output).is_ok() {
        return config(
            "restore_target_exists",
            "Restore requires a new, nonexistent directory",
        );
    }
    let name = output
        .file_name()
        .context("Restore output needs a directory name")?
        .to_str()
        .context("Non-UTF8 target")?;
    paths::safe_component(name)?;
    let parent = paths::existing(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let output = parent.join(name);
    let archive = paths::existing(archive)?;
    let before = engine::hash(&archive)?;
    engine.run(
        &[
            "t".into(),
            "-bd".into(),
            "--".into(),
            archive.as_os_str().into(),
        ],
        None,
    )?;
    let entries = list(engine, &archive)?;
    if entries.iter().any(|e| e.kind == "hardlink") {
        return config(
            "unsupported_archive_links",
            "Hard link entries are not supported",
        );
    }
    #[cfg(not(unix))]
    if entries.iter().any(|e| e.kind == "symlink") {
        return config(
            "unsupported_archive_links",
            "Symlink restore is not available on this platform",
        );
    }
    let leaves: HashSet<String> = entries
        .iter()
        .filter(|e| e.kind != "directory")
        .map(|e| paths::key(&e.path))
        .collect();
    for entry in &entries {
        let path = Path::new(&entry.path);
        for parent in path.ancestors().skip(1) {
            if leaves.contains(&paths::key(&parent.to_string_lossy())) {
                return config(
                    "unsafe_archive_path",
                    "An archive file or link cannot be another entry's parent",
                );
            }
        }
    }
    let total = entries
        .iter()
        .try_fold(0u64, |sum, e| sum.checked_add(e.bytes))
        .context("Archive declared size overflow")?;
    if total > fs2::available_space(&parent)? {
        return fail(5, "insufficient_space", "Not enough space for restore");
    }
    let temp = tempfile::Builder::new()
        .prefix(".smart-backup-restore-")
        .tempdir_in(&parent)?;
    paths::private_dir(temp.path())?;
    let mut links = vec![];
    for entry in &entries {
        engine::check_cancelled()?;
        let target = temp.path().join(paths::safe_entry(&entry.path)?);
        if entry.kind == "directory" {
            fs::create_dir_all(&target)?;
            continue;
        }
        fs::create_dir_all(target.parent().context("Entry parent")?)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        // 7-Zip only produces bytes on stdout; it never chooses an on-disk destination.
        engine.to_file(
            &[
                "x".into(),
                "-so".into(),
                "-spd".into(),
                "-bd".into(),
                "--".into(),
                archive.as_os_str().into(),
                entry.path.clone().into(),
            ],
            file.try_clone()?,
        )?;
        file.sync_all()?;
        if file.metadata()?.len() != entry.bytes {
            return fail(
                6,
                "entry_size_mismatch",
                "Extracted size differs from archive listing",
            );
        }
        if entry.kind == "symlink" {
            if entry.bytes > 4096 {
                return config("unsafe_archive_link", "Symlink target is too long");
            }
            let destination = fs::read_to_string(&target)?;
            // No absolute paths, parent components, drive prefixes, or platform aliases.
            // Links are materialized only after all ordinary writes have finished.
            let destination = paths::safe_entry(&destination)?;
            fs::remove_file(&target)?;
            links.push((target, destination));
        } else {
            set_time(&target, entry)?;
        }
    }
    for (target, destination) in links {
        #[cfg(unix)]
        std::os::unix::fs::symlink(destination, target)?;
        #[cfg(not(unix))]
        let _ = (target, destination);
    }
    for entry in entries.iter().rev().filter(|e| e.kind == "directory") {
        set_time(&temp.path().join(&entry.path), entry)?;
    }
    if engine::hash(&archive)? != before {
        return fail(
            6,
            "archive_changed",
            "Archive changed during restore; result not published",
        );
    }
    paths::rename_new(temp.path(), &output)?;
    paths::sync_dir(&parent)?;
    Ok(
        serde_json::json!({"output":output,"entries":entries.len(),"bytes":total,"status":"success"}),
    )
}
fn set_time(path: &Path, entry: &ArchiveEntry) -> Result<()> {
    if let Some(modified) = &entry.modified
        && let Ok(time) = chrono::NaiveDateTime::parse_from_str(modified, "%Y-%m-%d %H:%M:%S%.f")
    {
        let time = time.and_utc();
        filetime::set_file_mtime(
            path,
            filetime::FileTime::from_unix_time(time.timestamp(), time.timestamp_subsec_nanos()),
        )?;
    }
    Ok(())
}
