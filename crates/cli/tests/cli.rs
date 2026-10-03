use serde_json::Value;
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

fn cli(home: &Path, args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_smart-backup"))
        .args(["--home", home.to_str().unwrap(), "--json"])
        .args(args)
        .output()
        .unwrap();
    let json = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), json)
}

#[test]
fn job_can_be_created_and_read_back_with_unique_aliases() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("source");
    let out = tmp.path().join("backups");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&out).unwrap();
    fs::write(src.join("hello.txt"), "hello world").unwrap();
    let source = format!("Documents={}", src.display());
    let (code, created) = cli(
        &tmp.path().join("state"),
        &[
            "job",
            "create",
            "Daily",
            "--source",
            &source,
            "--output",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{created}");
    assert_eq!(created["data"]["name"], "Daily");
    let (_, shown) = cli(&tmp.path().join("state"), &["job", "show", "daily"]);
    assert_eq!(shown["data"]["id"], created["data"]["id"]);
    assert_eq!(shown["data"]["sources"][0]["alias"], "Documents");
}

fn engine() -> String {
    if let Ok(path) = std::env::var("SMART_BACKUP_7ZZ") {
        return path;
    }
    for name in ["7zz", "7z", "7z.exe"] {
        if Command::new(name)
            .arg("i")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return name.into();
        }
    }
    panic!("Install 7-Zip or set SMART_BACKUP_7ZZ; integration tests require a real engine");
}
struct Fixture {
    _tmp: TempDir,
    home: std::path::PathBuf,
    src: std::path::PathBuf,
    out: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("state");
        let src = tmp.path().join("source");
        let out = tmp.path().join("backups");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&out).unwrap();
        fs::write(src.join("hello.txt"), "hello world\n").unwrap();
        let source = format!("Documents={}", src.display());
        let (code, v) = cli(
            &home,
            &[
                "job",
                "create",
                "Daily",
                "--source",
                &source,
                "--output",
                out.to_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{v}");
        Self {
            _tmp: tmp,
            home,
            src,
            out,
        }
    }
    fn configure(&self) {
        let (code, v) = cli(&self.home, &["engine", "configure", "--path", &engine()]);
        assert_eq!(code, 0, "{v}");
    }
}
#[test]
fn snapshot_is_verified_and_contains_aliased_files_and_manifest() {
    let f = Fixture::new();
    f.configure();
    let (code, run) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 0, "{run}");
    assert_eq!(run["data"]["verification_state"], "verified");
    let archive = run["data"]["artifact"].as_str().unwrap();
    assert!(Path::new(archive).exists());
    assert!(Path::new(&format!("{archive}.sha256")).exists());
    let (code, verify) = cli(&f.home, &["verify", archive]);
    assert_eq!(code, 0, "{verify}");
    let output = Command::new(engine())
        .args(["x", "-so", archive, "Documents/hello.txt"])
        .output()
        .unwrap();
    assert_eq!(output.stdout, b"hello world\n");
    let output = Command::new(engine())
        .args(["x", "-so", archive, ".smart-backup/manifest.toml"])
        .output()
        .unwrap();
    let manifest = String::from_utf8(output.stdout).unwrap();
    assert!(manifest.contains("run_id"));
    assert!(!manifest.contains(f.src.to_str().unwrap()));
    assert_eq!(
        fs::read_to_string(f.src.join("hello.txt")).unwrap(),
        "hello world\n"
    );
}

#[test]
fn unchanged_run_skips_force_creates_new_snapshot_and_history_is_queryable() {
    let f = Fixture::new();
    f.configure();
    let (code, first) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 0, "{first}");
    let archive = first["data"]["artifact"].as_str().unwrap();
    let original = fs::read(archive).unwrap();
    let (code, second) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 2, "{second}");
    assert_eq!(second["reason_code"], "no_changes");
    let (code, forced) = cli(&f.home, &["run", "Daily", "--force"]);
    assert_eq!(code, 0, "{forced}");
    assert_ne!(forced["data"]["artifact"], first["data"]["artifact"]);
    assert_eq!(fs::read(archive).unwrap(), original);
    let (code, history) = cli(&f.home, &["history", "list"]);
    assert_eq!(code, 0, "{history}");
    assert_eq!(history["data"]["runs"].as_array().unwrap().len(), 3);
    assert_eq!(
        fs::read_dir(f.out.join(".smart-backup-staging"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn archive_can_be_browsed_and_restored_to_a_new_directory() {
    let f = Fixture::new();
    f.configure();
    fs::create_dir(f.src.join("empty")).unwrap();
    let (_, run) = cli(&f.home, &["run", "Daily"]);
    let archive = run["data"]["artifact"].as_str().unwrap();
    let (code, list) = cli(&f.home, &["archive", "list", archive]);
    assert_eq!(code, 0, "{list}");
    assert!(
        list["data"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "Documents/hello.txt")
    );
    let output = f._tmp.path().join("restore");
    let (code, restored) = cli(
        &f.home,
        &["restore", archive, "--output", output.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{restored}");
    assert_eq!(
        fs::read_to_string(output.join("Documents/hello.txt")).unwrap(),
        "hello world\n"
    );
    assert!(output.join("Documents/empty").is_dir());
    let (code, error) = cli(
        &f.home,
        &["restore", archive, "--output", output.to_str().unwrap()],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "restore_target_exists");
}

#[test]
fn dry_run_explains_filters_without_changing_history_or_baseline() {
    let f = Fixture::new();
    f.configure();
    fs::write(f.src.join("debug.log"), "exclude me").unwrap();
    fs::write(f.src.join("keep.log"), "keep me").unwrap();
    fs::write(f.src.join(".DS_Store"), "system metadata").unwrap();
    let (code, updated) = cli(
        &f.home,
        &[
            "job",
            "edit",
            "Daily",
            "--rule",
            "*.log",
            "--rule",
            "!keep.log",
        ],
    );
    assert_eq!(code, 0, "{updated}");
    let (code, preview) = cli(&f.home, &["run", "Daily", "--dry-run"]);
    assert_eq!(code, 0, "{preview}");
    assert_eq!(preview["data"]["files"], 2);
    assert!(
        preview["data"]["excluded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "Documents/debug.log")
    );
    let (_, history) = cli(&f.home, &["history", "list"]);
    assert_eq!(history["data"]["runs"].as_array().unwrap().len(), 0);
    assert_eq!(fs::read_dir(&f.out).unwrap().count(), 0);
    let (code, run) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 0, "{run}");
    let (_, listing) = cli(
        &f.home,
        &["archive", "list", run["data"]["artifact"].as_str().unwrap()],
    );
    let entries = listing["data"]["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["path"] == "Documents/keep.log"));
    assert!(!entries.iter().any(|e| e["path"] == "Documents/debug.log"));
}

#[test]
fn dangerous_archives_are_rejected_before_restore_target_is_created() {
    let f = Fixture::new();
    f.configure();
    for fixture in ["traversal", "absolute", "windows-drive", "collision"] {
        let archive =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{fixture}.zip"));
        let output = f._tmp.path().join(fixture);
        let (code, result) = cli(
            &f.home,
            &[
                "restore",
                archive.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ],
        );
        assert_ne!(code, 0, "{fixture}: {result}");
        assert!(!output.exists());
    }
    assert!(!f._tmp.path().join("escaped.txt").exists());
}

#[test]
fn unsafe_job_paths_aliases_and_duplicate_names_are_rejected() {
    let f = Fixture::new();
    let child = f.src.join("output");
    fs::create_dir(&child).unwrap();
    let source = format!("Docs={}", f.src.display());
    let (code, error) = cli(
        &f.home,
        &[
            "job",
            "create",
            "Recursive",
            "--source",
            &source,
            "--output",
            child.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "output_inside_source");
    let (code, error) = cli(
        &f.home,
        &[
            "job",
            "create",
            "DAILY",
            "--source",
            &source,
            "--output",
            f.out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "name_conflict");
    let second = format!("docs={}", f.src.display());
    let (code, error) = cli(
        &f.home,
        &[
            "job",
            "create",
            "Duplicate",
            "--source",
            &source,
            "--source",
            &second,
            "--output",
            f.out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "alias_conflict");
}

#[test]
fn changed_source_creates_a_new_snapshot_and_corruption_fails_verification() {
    let f = Fixture::new();
    f.configure();
    let (_, first) = cli(&f.home, &["run", "Daily"]);
    let original = first["data"]["artifact"].as_str().unwrap();
    fs::write(f.src.join("hello.txt"), "new longer content\n").unwrap();
    let (code, next) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 0, "{next}");
    assert_ne!(next["data"]["artifact"], first["data"]["artifact"]);
    fs::write(format!("{original}.sha256"), "0".repeat(64)).unwrap();
    let (code, error) = cli(&f.home, &["verify", original]);
    assert_eq!(code, 6, "{error}");
    assert_eq!(error["reason_code"], "checksum_mismatch");
}

#[test]
fn job_edits_preserve_comments_and_delete_preserves_archives() {
    let f = Fixture::new();
    f.configure();
    let (_, shown) = cli(&f.home, &["job", "show", "Daily"]);
    let job_file = f
        .home
        .join("jobs")
        .join(format!("{}.toml", shown["data"]["id"].as_str().unwrap()));
    let original = fs::read_to_string(&job_file).unwrap();
    fs::write(&job_file, format!("# important comment\n{original}")).unwrap();
    let (code, edited) = cli(&f.home, &["job", "edit", "Daily", "--name", "Renamed"]);
    assert_eq!(code, 0, "{edited}");
    assert_eq!(edited["data"]["id"], shown["data"]["id"]);
    assert!(
        fs::read_to_string(&job_file)
            .unwrap()
            .contains("# important comment")
    );
    let (_, run) = cli(&f.home, &["run", "Renamed"]);
    let archive = run["data"]["artifact"].as_str().unwrap();
    let (code, error) = cli(&f.home, &["job", "disable", "Renamed"]);
    assert_eq!(code, 0, "{error}");
    let (code, error) = cli(&f.home, &["run", "Renamed"]);
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "job_disabled");
    let (code, deleted) = cli(&f.home, &["job", "delete", "Renamed"]);
    assert_eq!(code, 0, "{deleted}");
    assert!(Path::new(archive).exists());
}

#[test]
fn broken_database_is_preserved_and_rebuilt_without_deleting_old_artifacts() {
    let f = Fixture::new();
    f.configure();
    let (_, first) = cli(&f.home, &["run", "Daily"]);
    let old = first["data"]["artifact"].as_str().unwrap();
    fs::write(f.home.join("state.db"), b"broken sqlite").unwrap();
    let (code, next) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 0, "{next}");
    assert_ne!(next["data"]["artifact"], first["data"]["artifact"]);
    assert!(Path::new(old).exists());
    assert!(fs::read_dir(&f.home).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("state.corrupt-")
    }));
}

#[test]
fn volume_mismatch_and_unsupported_config_fail_closed() {
    let f = Fixture::new();
    f.configure();
    let (_, shown) = cli(&f.home, &["job", "show", "Daily"]);
    let path = f
        .home
        .join("jobs")
        .join(format!("{}.toml", shown["data"]["id"].as_str().unwrap()));
    let original = fs::read_to_string(&path).unwrap();
    let actual = shown["data"]["output_volume"]["primary_id"]
        .as_str()
        .unwrap();
    fs::write(&path, original.replace(actual, "wrong-volume")).unwrap();
    let (code, error) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 5, "{error}");
    assert_eq!(error["reason_code"], "volume_identity_mismatch");
    fs::write(
        &path,
        original.replace("verification = \"full\"", "verification = \"off\""),
    )
    .unwrap();
    let (code, error) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "unsupported_in_version");
    assert_eq!(fs::read_dir(&f.out).unwrap().count(), 0);
}

#[test]
fn unicode_and_at_prefixed_names_round_trip_as_literal_paths() {
    let f = Fixture::new();
    f.configure();
    fs::write(f.src.join("猫咪.txt"), "喵，备份内容🐾").unwrap();
    let source = format!("@资料={}", f.src.display());
    let (code, job) = cli(
        &f.home,
        &[
            "job",
            "create",
            "Unicode",
            "--source",
            &source,
            "--output",
            f.out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{job}");
    let (_, run) = cli(&f.home, &["run", "Unicode"]);
    let archive = run["data"]["artifact"].as_str().unwrap();
    let out = f._tmp.path().join("unicode-restore");
    let (code, result) = cli(
        &f.home,
        &["restore", archive, "--output", out.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(
        fs::read_to_string(out.join("@资料/猫咪.txt")).unwrap(),
        "喵，备份内容🐾"
    );
}

#[test]
fn invalid_cli_arguments_still_return_versioned_json() {
    let tmp = TempDir::new().unwrap();
    let (code, error) = cli(tmp.path(), &["run"]);
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["schema_version"], 1);
    assert_eq!(error["reason_code"], "invalid_arguments");
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_run_without_publishing_artifacts() {
    use std::{
        io::Write,
        process::Stdio,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    f.configure();
    let mut file = fs::File::create(f.src.join("big.bin")).unwrap();
    let data: Vec<u8> = (0..1024 * 1024)
        .map(|i| ((i * 31 + i / 7) % 251) as u8)
        .collect();
    for _ in 0..32 {
        file.write_all(&data).unwrap();
    }
    drop(file);
    let child = Command::new(env!("CARGO_BIN_EXE_smart-backup"))
        .args(["--home", f.home.to_str().unwrap(), "--json", "run", "Daily"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    let mut observed = false;
    while Instant::now() < until {
        if f.home.join("logs").exists()
            && fs::read_dir(f.home.join("logs"))
                .unwrap()
                .flatten()
                .any(|e| {
                    fs::read_to_string(e.path())
                        .unwrap_or_default()
                        .contains("archiving")
                })
        {
            observed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(observed, "Run did not reach archiving");
    Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let (_, history) = cli(&f.home, &["history", "list"]);
    assert_eq!(history["data"]["runs"][0]["status"], "cancelled");
    assert!(
        !fs::read_dir(&f.out)
            .unwrap()
            .flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "7z"))
    );
}

#[cfg(unix)]
#[test]
fn safe_relative_symlink_restores_but_escaping_link_does_not() {
    let f = Fixture::new();
    f.configure();
    std::os::unix::fs::symlink("hello.txt", f.src.join("link")).unwrap();
    let (_, run) = cli(&f.home, &["run", "Daily"]);
    let archive = run["data"]["artifact"].as_str().unwrap();
    let output = f._tmp.path().join("links");
    let (code, result) = cli(
        &f.home,
        &["restore", archive, "--output", output.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(
        fs::read_link(output.join("Documents/link")).unwrap(),
        Path::new("hello.txt")
    );
    fs::remove_file(f.src.join("link")).unwrap();
    std::os::unix::fs::symlink("../../escape", f.src.join("link")).unwrap();
    let (_, run) = cli(&f.home, &["run", "Daily"]);
    let archive = run["data"]["artifact"].as_str().unwrap();
    let output = f._tmp.path().join("bad-links");
    let (code, _) = cli(
        &f.home,
        &["restore", archive, "--output", output.to_str().unwrap()],
    );
    assert_ne!(code, 0);
    assert!(!output.exists());
}

#[test]
fn dry_run_does_not_rebuild_or_mutate_a_damaged_history_database() {
    let f = Fixture::new();
    f.configure();
    fs::write(f.home.join("state.db"), b"diagnostic evidence").unwrap();
    let (code, error) = cli(&f.home, &["run", "Daily", "--dry-run"]);
    assert_eq!(code, 5, "{error}");
    assert_eq!(error["reason_code"], "database_corrupt");
    assert_eq!(
        fs::read(f.home.join("state.db")).unwrap(),
        b"diagnostic evidence"
    );
}

#[cfg(unix)]
#[test]
fn nonportable_source_names_and_root_symlinks_fail_explicitly() {
    let f = Fixture::new();
    f.configure();
    fs::write(f.src.join("a\\b.txt"), "must not rename me").unwrap();
    let (code, error) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "unsafe_path");
    assert!(
        !fs::read_dir(&f.out)
            .unwrap()
            .flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "7z"))
    );
    let link = f._tmp.path().join("source-link");
    std::os::unix::fs::symlink(&f.src, &link).unwrap();
    let source = format!("Docs={}", link.display());
    let (code, error) = cli(
        &f.home,
        &[
            "job",
            "create",
            "Link",
            "--source",
            &source,
            "--output",
            f.out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "unsupported_source_symlink");
}

#[test]
fn explicit_include_overrides_builtin_cache_tag_exclusion() {
    let f = Fixture::new();
    f.configure();
    let cache = f.src.join("cache");
    fs::create_dir(&cache).unwrap();
    fs::write(
        cache.join("CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55\n",
    )
    .unwrap();
    fs::write(cache.join("important.txt"), "include this explicitly").unwrap();
    let (_, before) = cli(&f.home, &["run", "Daily", "--dry-run"]);
    assert!(
        before["data"]["excluded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "Documents/cache")
    );
    let (code, edit) = cli(
        &f.home,
        &[
            "job",
            "edit",
            "Daily",
            "--rule",
            "!cache/",
            "--rule",
            "!cache/**",
        ],
    );
    assert_eq!(code, 0, "{edit}");
    let (_, preview) = cli(&f.home, &["run", "Daily", "--dry-run"]);
    assert!(
        preview["data"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "Documents/cache/important.txt"),
        "{preview}"
    );
    let (_, run) = cli(&f.home, &["run", "Daily"]);
    let (_, listing) = cli(
        &f.home,
        &["archive", "list", run["data"]["artifact"].as_str().unwrap()],
    );
    assert!(
        listing["data"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "Documents/cache/important.txt")
    );
}

#[test]
fn multiple_sources_keep_same_named_files_in_separate_aliases() {
    let f = Fixture::new();
    f.configure();
    let second = f._tmp.path().join("second");
    fs::create_dir(&second).unwrap();
    fs::write(second.join("hello.txt"), "second source").unwrap();
    let a = format!("First={}", f.src.display());
    let b = format!("Second={}", second.display());
    let (code, created) = cli(
        &f.home,
        &[
            "job",
            "create",
            "Multi",
            "--source",
            &a,
            "--source",
            &b,
            "--output",
            f.out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{created}");
    let (_, run) = cli(&f.home, &["run", "Multi"]);
    let archive = run["data"]["artifact"].as_str().unwrap();
    let output = f._tmp.path().join("multi-restore");
    let (code, result) = cli(
        &f.home,
        &["restore", archive, "--output", output.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(
        fs::read_to_string(output.join("First/hello.txt")).unwrap(),
        "hello world\n"
    );
    assert_eq!(
        fs::read_to_string(output.join("Second/hello.txt")).unwrap(),
        "second source"
    );
    let original = fs::metadata(f.src.join("hello.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let restored = fs::metadata(output.join("First/hello.txt"))
        .unwrap()
        .modified()
        .unwrap();
    assert!(
        original.duration_since(restored).unwrap_or_default() < std::time::Duration::from_secs(1)
    );
    assert!(
        restored.duration_since(original).unwrap_or_default() < std::time::Duration::from_secs(1)
    );
}

#[test]
fn changed_engine_hash_blocks_run_and_records_failure() {
    let f = Fixture::new();
    f.configure();
    let path = f.home.join("config.toml");
    let text = fs::read_to_string(&path).unwrap();
    let line = text.lines().find(|l| l.starts_with("sha256 = ")).unwrap();
    fs::write(
        &path,
        text.replace(line, &format!("sha256 = \"{}\"", "0".repeat(64))),
    )
    .unwrap();
    let (code, error) = cli(&f.home, &["run", "Daily"]);
    assert_eq!(code, 5, "{error}");
    assert_eq!(error["reason_code"], "engine_hash_mismatch");
    let (_, history) = cli(&f.home, &["history", "list"]);
    assert_eq!(history["data"]["runs"][0]["status"], "failed");
    assert_eq!(fs::read_dir(&f.out).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn trailing_separators_cannot_hide_a_source_root_symlink() {
    let f = Fixture::new();
    let link = f._tmp.path().join("link");
    std::os::unix::fs::symlink(&f.src, &link).unwrap();
    for suffix in ["/", "/.", "//./"] {
        let source = format!("Docs={}{}", link.display(), suffix);
        let (code, error) = cli(
            &f.home,
            &[
                "job",
                "create",
                "Link",
                "--source",
                &source,
                "--output",
                f.out.to_str().unwrap(),
            ],
        );
        assert_eq!(code, 4, "{source}: {error}");
        assert_eq!(error["reason_code"], "unsupported_source_symlink");
    }
}

fn tree_snapshot(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>, std::time::SystemTime)> {
    fn visit(
        root: &Path,
        path: &Path,
        entries: &mut Vec<(std::path::PathBuf, Vec<u8>, std::time::SystemTime)>,
    ) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(path).unwrap()
        } else {
            vec![]
        };
        entries.push((
            path.strip_prefix(root).unwrap().to_owned(),
            bytes,
            metadata.modified().unwrap(),
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = vec![];
    visit(root, root, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn state_inside_source_is_rejected_before_any_startup_writes() {
    let f = Fixture::new();
    let (_, shown) = cli(&f.home, &["job", "show", "Daily"]);
    let job = f
        .home
        .join("jobs")
        .join(format!("{}.toml", shown["data"]["id"].as_str().unwrap()));
    let text = fs::read_to_string(&job).unwrap();
    // Keep output outside the new source while placing state inside it.
    let container = f._tmp.path().join("container");
    fs::create_dir(&container).unwrap();
    let home = container.join("state");
    fs::rename(&f.home, &home).unwrap();
    let job = home.join("jobs").join(job.file_name().unwrap());
    let old_path = shown["data"]["sources"][0]["path"].as_str().unwrap();
    // Both fixture paths are on the same volume; retain its recorded identity.
    fs::write(
        job,
        text.replace(
            old_path,
            container.canonicalize().unwrap().to_str().unwrap(),
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let before = tree_snapshot(&container);
    for args in [
        vec!["run", "Daily"],
        vec!["history", "list"],
        vec!["config", "validate"],
    ] {
        let (code, error) = cli(&home, &args);
        assert_eq!(code, 4, "{error}");
        assert_eq!(error["reason_code"], "state_inside_source");
        assert!(
            tree_snapshot(&container) == before,
            "Rejected command changed source tree"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&home).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }
}

#[test]
fn job_creation_does_not_initialize_state_inside_its_source() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let output = temp.path().join("output");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&output).unwrap();
    fs::write(source.join("keep.txt"), b"unchanged").unwrap();
    let before = tree_snapshot(&source);
    let home = source.join("new/nested/state");
    let (code, error) = cli(
        &home,
        &[
            "job",
            "create",
            "Unsafe",
            "--source",
            &format!("Docs={}", source.display()),
            "--output",
            output.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "state_inside_source");
    assert!(
        tree_snapshot(&source) == before,
        "Creating an invalid Job changed its source"
    );
}

#[cfg(unix)]
#[test]
fn state_source_guard_resolves_symlink_ancestors_and_parent_components() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let output = temp.path().join("output");
    fs::create_dir_all(source.join("child")).unwrap();
    fs::create_dir(&output).unwrap();
    std::os::unix::fs::symlink(source.join("child"), temp.path().join("alias")).unwrap();
    let before = tree_snapshot(&source);
    let home = temp.path().join("alias/../new/state");
    let (code, error) = cli(
        &home,
        &[
            "job",
            "create",
            "Unsafe",
            "--source",
            &format!("Docs={}", source.display()),
            "--output",
            output.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 4, "{error}");
    assert_eq!(error["reason_code"], "state_inside_source");
    assert!(tree_snapshot(&source) == before);
}
