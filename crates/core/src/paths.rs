use crate::{
    error::{config, fail},
    model::VolumeIdentity,
};
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;

pub fn key(value: &str) -> String {
    use caseless::Caseless;
    value.nfkc().default_case_fold().nfkc().collect()
}
pub fn safe_component(name: &str) -> Result<()> {
    let upper = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || reserved.contains(&upper.as_str())
    {
        return config("unsafe_path", format!("Unsafe portable name: {name:?}"));
    }
    Ok(())
}
pub fn safe_entry(name: &str) -> Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return config("unsafe_archive_path", "Archive path must be relative");
    }
    for part in normalized.split('/') {
        safe_component(part)?;
    }
    let p = PathBuf::from(normalized);
    if !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return config("unsafe_archive_path", "Archive contains traversal");
    }
    Ok(p)
}
pub fn existing(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("Path unavailable: {}", path.display()))
}
/// Resolve existing ancestors without creating missing path components.
/// Resolve symlinks before processing `..`, including for a not-yet-created home.
pub fn prospective(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) => {
                // A Windows device/drive prefix is not a complete path until RootDir.
                resolved.push(component.as_os_str());
                continue;
            }
            Component::CurDir => continue,
            Component::ParentDir => {
                resolved.pop();
            }
            _ => resolved.push(component.as_os_str()),
        }
        match fs::canonicalize(&resolved) {
            Ok(path) => resolved = path,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(resolved)
}
/// Resolve the writable state location without relaxing the ban on a linked home.
pub fn state_home(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    if let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) {
        // Resolve missing/parent components before checking the final component.
        let candidate = prospective(parent)?.join(name);
        if fs::symlink_metadata(&candidate).is_ok_and(|m| m.file_type().is_symlink()) {
            return fail(
                5,
                "unsafe_state_directory",
                "State/staging path must be a real directory",
            );
        }
    }
    prospective(&absolute)
}
pub fn check_state_source(home: &Path, source: &Path) -> Result<()> {
    let source = prospective(source)?;
    if home.starts_with(&source) {
        return config(
            "state_inside_source",
            "State directory cannot be inside a source (it changes during runs)",
        );
    }
    if source.starts_with(home) {
        return config(
            "state_overlap",
            "Sources cannot be inside the state directory",
        );
    }
    Ok(())
}
pub fn private_dir(path: &Path) -> Result<()> {
    if path.exists() {
        if fs::symlink_metadata(path)?.file_type().is_symlink() || !path.is_dir() {
            return fail(
                5,
                "unsafe_state_directory",
                "State/staging path must be a real directory",
            );
        }
    } else {
        fs::create_dir_all(path)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
pub fn volume(path: &Path) -> Result<VolumeIdentity> {
    platform_volume(path)
}
#[cfg(target_os = "macos")]
fn platform_volume(path: &Path) -> Result<VolumeIdentity> {
    use std::{
        ffi::{CStr, CString},
        mem::MaybeUninit,
        os::unix::ffi::OsStrExt,
        process::Command,
    };
    let name = CString::new(path.as_os_str().as_bytes())?;
    let mut stat = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: valid NUL-terminated path and writable statfs storage.
    if unsafe { libc::statfs(name.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: successful statfs initialized the structure and its C strings.
    let stat = unsafe { stat.assume_init() };
    let mount = unsafe { CStr::from_ptr(stat.f_mntonname.as_ptr()) }.to_string_lossy();
    let output = Command::new("/usr/sbin/diskutil")
        .args(["info", "-plist", &mount])
        .output()?;
    let data = plist::Value::from_reader_xml(output.stdout.as_slice())?;
    let get = |name: &str| {
        data.as_dictionary()
            .and_then(|d| d.get(name))
            .and_then(|v| v.as_string())
            .map(str::to_owned)
    };
    match (get("VolumeUUID"), get("FilesystemType")) {
        (Some(primary_id), Some(fs_type)) if output.status.success() => Ok(VolumeIdentity {
            kind: "macos_volume_uuid".into(),
            primary_id,
            fs_type,
        }),
        _ => fail(
            5,
            "unsupported_volume_identity",
            format!("No stable volume UUID for {}", path.display()),
        ),
    }
}
#[cfg(target_os = "linux")]
fn platform_volume(path: &Path) -> Result<VolumeIdentity> {
    let out = std::process::Command::new("findmnt")
        .args(["--json", "--target"])
        .arg(path)
        .args(["--output", "UUID,FSTYPE,SOURCE"])
        .output()?;
    let value: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let v = &value["filesystems"][0];
    let fs_type = v["fstype"].as_str().unwrap_or("");
    let uuid = v["uuid"].as_str().unwrap_or("");
    if !uuid.is_empty() {
        return Ok(VolumeIdentity {
            kind: "linux_fs_uuid".into(),
            primary_id: uuid.into(),
            fs_type: fs_type.into(),
        });
    }
    fail(
        5,
        "unsupported_volume_identity",
        format!(
            "No stable filesystem UUID for {}; network/overlay volumes are not supported by this prototype",
            path.display()
        ),
    )
}
#[cfg(target_os = "windows")]
fn platform_volume(path: &Path) -> Result<VolumeIdentity> {
    // The path is data in the child environment, never interpolated into PowerShell code.
    let out = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; $v=Get-Volume -FilePath $env:SMART_BACKUP_VOLUME_PATH; @{id=$v.UniqueId;fs=$v.FileSystemType.ToString()} | ConvertTo-Json -Compress"]).env("SMART_BACKUP_VOLUME_PATH", path).output()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    match (v["id"].as_str(), v["fs"].as_str()) {
        (Some(id), Some(fs_type)) if out.status.success() && !id.is_empty() => Ok(VolumeIdentity {
            kind: "windows_volume_guid".into(),
            primary_id: id.into(),
            fs_type: fs_type.into(),
        }),
        _ => fail(
            5,
            "unsupported_volume_identity",
            "No stable Windows volume identity",
        ),
    }
}
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_volume(_path: &Path) -> Result<VolumeIdentity> {
    fail(5, "unsupported_platform", "Platform unsupported")
}

pub fn check_volume(path: &Path, expected: &VolumeIdentity) -> Result<()> {
    if &volume(path)? != expected {
        return fail(
            5,
            "volume_identity_mismatch",
            format!("Volume changed at {}", path.display()),
        );
    }
    Ok(())
}

/// Atomically publish a directory without replacing any existing name.
pub fn rename_new(from: &Path, to: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(from.as_os_str().as_bytes())?;
        let to = CString::new(to.as_os_str().as_bytes())?;
        // SAFETY: both paths are valid NUL-terminated strings; no-replace is mandatory.
        #[cfg(target_os = "macos")]
        let result = unsafe {
            libc::renameatx_np(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let result = -1;
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: valid UTF-16 NUL-terminated paths. MoveFileW never replaces a target.
        if unsafe { windows_sys::Win32::Storage::FileSystem::MoveFileW(from.as_ptr(), to.as_ptr()) }
            == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}

/// Normalize separators and trailing `.` without following the last source component.
/// lstat("link/") follows the target on Unix; lstat("link") does not.
pub fn source_candidate(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return config("invalid_source", "Source path cannot be empty");
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let normalized: PathBuf = absolute
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    if fs::symlink_metadata(&normalized).is_ok_and(|m| m.file_type().is_symlink()) {
        return config(
            "unsupported_source_symlink",
            "Source root links are unsupported; select a real file/directory. Links inside it are preserved.",
        );
    }
    Ok(normalized)
}
