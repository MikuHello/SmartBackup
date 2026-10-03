use crate::{
    error::{config, fail},
    model::{Filters, Job, Source},
    paths,
};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn validate(job: &Job, home: &Path, check_available: bool) -> Result<()> {
    if job.schema_version != 1 {
        return config("unsupported_schema", "Only Job schema 1 is supported");
    }
    if Uuid::parse_str(&job.id).is_err() {
        return config("invalid_job_id", "Job ID must be a UUID");
    }
    if job.name.trim().is_empty() || job.name.chars().any(char::is_control) {
        return config(
            "invalid_name",
            "Job name cannot be empty or contain control characters",
        );
    }
    if job.format != "7z"
        || job.verification != "full"
        || job.change_detection != "metadata"
        || !job.extra.is_empty()
    {
        return config(
            "unsupported_in_version",
            "0.1.0 supports only 7z, full verification, metadata detection and documented fields",
        );
    }
    if job.sources.is_empty() {
        return config("no_sources", "At least one source is required");
    }
    let mut aliases = HashSet::new();
    for source in &job.sources {
        paths::safe_component(&source.alias)?;
        if paths::key(&source.alias) == ".smart-backup"
            || !aliases.insert(paths::key(&source.alias))
        {
            return config(
                "alias_conflict",
                "Source aliases must be unique and cannot be .smart-backup",
            );
        }
        if source.follow_symlinks {
            return config(
                "unsupported_in_version",
                "Following symlinks is not supported in this prototype",
            );
        }
        if !source.path.is_absolute() || !job.output.is_absolute() {
            return config(
                "absolute_path_required",
                "Source and output must be absolute paths",
            );
        }
        if check_available {
            if !source.path.exists() && !source.required {
                continue;
            }
            let root = paths::existing(&source.path).map_err(|e| crate::error::Failure {
                exit: 5,
                reason: "source_unavailable",
                message: e.to_string(),
            })?;
            paths::check_volume(&root, &source.volume)?;
            let output = paths::existing(&job.output)?;
            if output.starts_with(&root) {
                return config("output_inside_source", "Output cannot be inside a source");
            }
            if home.starts_with(&root) {
                return config(
                    "state_inside_source",
                    "State directory cannot be inside a source (it changes during runs)",
                );
            }
            if root.starts_with(home) || output.starts_with(home) {
                return config(
                    "state_overlap",
                    "Sources and output cannot be inside the state directory",
                );
            }
        }
    }
    if check_available {
        if !job.output.is_dir() {
            return fail(
                5,
                "output_unavailable",
                "Output directory must already exist on the intended volume",
            );
        }
        paths::check_volume(&job.output, &job.output_volume)?;
    }
    Ok(())
}

pub fn create(
    home: &Path,
    name: &str,
    sources: &[String],
    output: &Path,
    rules: &[String],
    follow_gitignore: bool,
) -> Result<Job> {
    let output = paths::existing(output)?;
    let mut result = Vec::new();
    for value in sources {
        let (alias, source) = value
            .split_once('=')
            .context("Source must use ALIAS=PATH syntax")?;
        let source = paths::existing(Path::new(source))?;
        result.push(Source {
            alias: alias.to_owned(),
            volume: paths::volume(&source)?,
            path: source,
            required: true,
            follow_symlinks: false,
            cross_filesystems: false,
        });
    }
    let job = Job {
        schema_version: 1,
        id: Uuid::new_v4().to_string(),
        name: name.trim().to_owned(),
        description: String::new(),
        enabled: true,
        sources: result,
        output_volume: paths::volume(&output)?,
        output,
        format: "7z".into(),
        verification: "full".into(),
        change_detection: "metadata".into(),
        filters: Filters {
            rules: rules.to_vec(),
            follow_gitignore,
            ..Default::default()
        },
        extra: BTreeMap::new(),
    };
    validate(&job, home, true)?;
    if list(home)?
        .0
        .iter()
        .any(|j| paths::key(&j.name) == paths::key(&job.name))
    {
        return config("name_conflict", "A Job with this name already exists");
    }
    write_new(
        &home.join("jobs").join(format!("{}.toml", job.id)),
        toml::to_string_pretty(&job)?.as_bytes(),
    )?;
    Ok(job)
}
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("File needs a parent directory")?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(path).map_err(|e| e.error)?;
    paths::sync_dir(parent)?;
    Ok(())
}
pub fn list(home: &Path) -> Result<(Vec<Job>, Vec<String>)> {
    let mut jobs = vec![];
    let mut invalid = vec![];
    for entry in fs::read_dir(home.join("jobs"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|s| s == "toml") {
            let parsed = fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|s| toml::from_str::<Job>(&s).map_err(Into::into));
            match parsed {
                Ok(job) => jobs.push(job),
                Err(_) => invalid.push(path.display().to_string()),
            }
        }
    }
    jobs.sort_by_key(|j| paths::key(&j.name));
    Ok((jobs, invalid))
}
pub fn get(home: &Path, selector: &str) -> Result<Job> {
    let matches: Vec<Job> = list(home)?
        .0
        .into_iter()
        .filter(|j| j.id == selector || paths::key(&j.name) == paths::key(selector))
        .collect();
    match matches.len() {
        0 => config("job_not_found", format!("Job not found: {selector}")),
        1 => {
            let job = matches.into_iter().next().unwrap();
            validate(&job, home, false)?;
            Ok(job)
        }
        _ => config(
            "ambiguous_job",
            "Duplicate Job identities/names; repair configuration",
        ),
    }
}
pub fn file(home: &Path, job: &Job) -> PathBuf {
    home.join("jobs").join(format!("{}.toml", job.id))
}

pub fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("File needs a parent")?;
    let previous = fs::read(path)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    if fs::read(path)? != previous {
        return config(
            "config_changed",
            "Configuration was edited externally; refusing overwrite",
        );
    }
    fs::write(path.with_extension("toml.bak"), previous)?;
    tmp.persist(path).map_err(|e| e.error)?;
    paths::sync_dir(parent)?;
    Ok(())
}

#[derive(Default)]
pub struct Changes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub rules: Option<Vec<String>>,
}
pub fn edit(home: &Path, selector: &str, changes: Changes) -> Result<Job> {
    let job = get(home, selector)?;
    let path = file(home, &job);
    let original = fs::read_to_string(&path)?;
    let mut document: toml_edit::DocumentMut = original.parse()?;
    if let Some(name) = changes.name {
        if list(home)?
            .0
            .iter()
            .any(|j| j.id != job.id && paths::key(&j.name) == paths::key(&name))
        {
            return config("name_conflict", "A Job with this name already exists");
        }
        document["name"] = toml_edit::value(name);
    }
    if let Some(description) = changes.description {
        document["description"] = toml_edit::value(description);
    }
    if let Some(enabled) = changes.enabled {
        document["enabled"] = toml_edit::value(enabled);
    }
    if let Some(rules) = changes.rules {
        let mut array = toml_edit::Array::new();
        for rule in rules {
            array.push(rule);
        }
        document["filters"]["rules"] = toml_edit::value(array);
    }
    let text = document.to_string();
    let edited: Job = toml::from_str(&text)?;
    validate(&edited, home, false)?;
    if fs::read_to_string(&path)? != original {
        return config("config_changed", "Job was edited externally; retry");
    }
    replace(&path, text.as_bytes())?;
    Ok(edited)
}
pub fn delete(home: &Path, selector: &str) -> Result<Job> {
    let job = get(home, selector)?;
    fs::remove_file(file(home, &job))?;
    let backup = file(home, &job).with_extension("toml.bak");
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    paths::sync_dir(&home.join("jobs"))?;
    Ok(job)
}
