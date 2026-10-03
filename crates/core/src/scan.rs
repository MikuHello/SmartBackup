use crate::{
    engine,
    error::{config, fail},
    model::Job,
    paths,
};
use anyhow::{Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    pub modified_ns: u128,
    pub link_target: Option<String>,
    #[serde(skip)]
    pub source: PathBuf,
}
#[derive(Debug, Clone, Serialize)]
pub struct Excluded {
    pub path: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub excluded: Vec<Excluded>,
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
    pub fingerprint: String,
    pub rules_hash: String,
    pub warnings: Vec<String>,
    pub available_sources: usize,
}
fn matcher(root: &Path, rules: &[String]) -> Result<Gitignore> {
    let mut builder = GitignoreBuilder::new(root);
    for rule in rules {
        builder
            .add_line(None, rule)
            .map_err(|e| crate::error::Failure {
                exit: 4,
                reason: "invalid_filter",
                message: e.to_string(),
            })?;
    }
    Ok(builder.build()?)
}
fn decision(matcher: &Gitignore, path: &Path, dir: bool) -> Option<(bool, String)> {
    let m = matcher.matched(path, dir);
    m.inner()
        .map(|rule| (m.is_ignore(), rule.original().to_owned()))
}
pub fn scan(job: &Job) -> Result<Scan> {
    let mut result = Scan {
        entries: vec![],
        excluded: vec![],
        files: 0,
        directories: 0,
        bytes: 0,
        fingerprint: String::new(),
        rules_hash: String::new(),
        warnings: vec![],
        available_sources: 0,
    };
    let mut rules_bytes = serde_json::to_vec(&job.filters)?;
    let mut external = vec![];
    for path in &job.filters.rule_files {
        match fs::read_to_string(path) {
            Ok(content) => {
                rules_bytes.extend_from_slice(content.as_bytes());
                external.extend(content.lines().map(str::to_owned));
            }
            Err(_) => result
                .warnings
                .push(format!("rule_file_unavailable: {}", path.display())),
        }
    }
    let mut names = HashSet::new();
    for source in &job.sources {
        if !source.path.exists() && !source.required {
            result
                .warnings
                .push(format!("optional_source_unavailable: {}", source.alias));
            continue;
        }
        result.available_sources += 1;
        let root = if source.path.is_dir() {
            source.path.as_path()
        } else {
            source.path.parent().context("Source parent")?
        };
        let defaults = if job.filters.builtin {
            vec!["desktop.ini".into(), "Thumbs.db".into(), ".DS_Store".into()]
        } else {
            vec![]
        };
        let builtin = matcher(root, &defaults)?;
        let file_rules = matcher(root, &external)?;
        let inline = matcher(root, &job.filters.rules)?;
        let mut ignores: HashMap<PathBuf, Gitignore> = HashMap::new();
        let mut walk = walkdir::WalkDir::new(&source.path)
            .follow_links(false)
            .same_file_system(!source.cross_filesystems)
            .sort_by_file_name()
            .into_iter();
        while let Some(next) = walk.next() {
            engine::check_cancelled()?;
            let item = next?;
            let path = item.path();
            let relative = if path == source.path {
                source.alias.clone()
            } else {
                format!(
                    "{}/{}",
                    source.alias,
                    path.strip_prefix(&source.path)?
                        .to_str()
                        .context("Non-UTF8 filenames unsupported")?
                        .replace('\\', "/")
                )
            };
            paths::safe_entry(&relative)?;
            let dir = item.file_type().is_dir();
            let mut matched = decision(&builtin, path, dir);
            if job.filters.follow_gitignore {
                let mut ancestors: Vec<&Path> = path
                    .parent()
                    .unwrap_or(root)
                    .ancestors()
                    .take_while(|p| p.starts_with(root))
                    .collect();
                ancestors.reverse();
                for ancestor in ancestors {
                    if !ignores.contains_key(ancestor) {
                        let ignore_file = ancestor.join(".gitignore");
                        let content = match fs::read_to_string(&ignore_file) {
                            Ok(s) => s,
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                            Err(e) => return Err(e.into()),
                        };
                        rules_bytes.extend_from_slice(content.as_bytes());
                        ignores.insert(
                            ancestor.to_path_buf(),
                            matcher(
                                ancestor,
                                &content.lines().map(str::to_owned).collect::<Vec<_>>(),
                            )?,
                        );
                    }
                    if let Some(value) = decision(&ignores[ancestor], path, dir) {
                        matched = Some(value);
                    }
                }
            }
            if let Some(value) = decision(&file_rules, path, dir) {
                matched = Some(value);
            }
            if let Some(value) = decision(&inline, path, dir) {
                matched = Some(value);
            }
            if path != source.path
                && let Some((true, rule)) = matched
            {
                result.excluded.push(Excluded {
                    path: relative,
                    reason: rule,
                });
                if dir {
                    walk.skip_current_dir();
                }
                continue;
            }
            if job.filters.builtin
                && dir
                && fs::read(path.join("CACHEDIR.TAG")).is_ok_and(|data| {
                    data.starts_with(b"Signature: 8a477f597d28d172789f06886806bc55")
                })
            {
                result.excluded.push(Excluded {
                    path: relative,
                    reason: "CACHEDIR.TAG cache directory".into(),
                });
                walk.skip_current_dir();
                continue;
            }
            if !names.insert(paths::key(&relative)) {
                return config(
                    "archive_path_collision",
                    format!("Portable archive name collision: {relative}"),
                );
            }
            let metadata = fs::symlink_metadata(path)?;
            let (kind, link_target, bytes) = if metadata.file_type().is_symlink() {
                let target = fs::read_link(path)?;
                (
                    "symlink",
                    Some(target.to_str().context("Non-UTF8 link target")?.to_owned()),
                    0,
                )
            } else if metadata.is_dir() {
                ("directory", None, 0)
            } else if metadata.is_file() {
                ("file", None, metadata.len())
            } else {
                return fail(
                    5,
                    "unsupported_file_type",
                    format!("Special file cannot be backed up: {relative}"),
                );
            };
            let modified_ns = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
            if kind == "file" {
                result.files += 1;
                result.bytes += bytes;
            } else if kind == "directory" {
                result.directories += 1;
            }
            result.entries.push(Entry {
                path: relative,
                kind: kind.into(),
                bytes,
                modified_ns,
                source: path.to_owned(),
                link_target,
            });
        }
    }
    result.entries.sort_by(|a, b| a.path.cmp(&b.path));
    result.rules_hash = engine::digest(&rules_bytes);
    result.fingerprint = engine::digest(&serde_json::to_vec(&(
        &result.entries,
        &result.rules_hash,
        &job.sources,
    ))?);
    Ok(result)
}
