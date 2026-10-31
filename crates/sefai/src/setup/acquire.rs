//! Explicit acquisition of one pinned catalog entry.
//!
//! A preview lists every file, its size, whether a verified copy is already present and the
//! only host contacted. Downloading happens only after the user confirms that preview: files
//! are written into a staging directory beside the destination, each is checked against its
//! pinned size and SHA-256, and the complete bundle is renamed into place. An interrupted
//! acquisition leaves verified files in staging for the next attempt and never a half-valid
//! destination. Nothing from the repository is executed.

use crate::workspace::CatalogEntry;
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

const HOST: &str = "huggingface.co";
const CHUNK_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// Every body gets a minute plus the time it needs at this slowest accepted rate.
const BODY_BYTES_PER_SECOND_MIN: u64 = 256 * 1024;
const REDIRECTS_MAX: u32 = 5;

/// Where an entry's files go: `destination/ID`, staged in `destination/.staging-ID`.
pub fn directory(destination: &Path, entry: &CatalogEntry) -> PathBuf {
    destination.join(&entry.id)
}

fn staging(destination: &Path, entry: &CatalogEntry) -> PathBuf {
    destination.join(format!(".staging-{}", entry.id))
}

/// Describes exactly what acquiring `entry` would download, without network access.
pub fn preview(entry: &CatalogEntry, destination: &Path, disk_free_bytes: Option<u64>) -> String {
    let target = directory(destination, entry);
    let mut text = format!(
        "acquire {} · {}@{} · license {} · format {}\nhost https://{HOST} and the storage hosts it redirects to over https\ndestination {}\n",
        entry.id,
        entry.repository,
        entry.revision,
        entry.license,
        entry.format.name(),
        target.display()
    );
    let mut missing = 0u64;
    for file in &entry.files {
        let present = [
            target.join(&file.path),
            staging(destination, entry).join(&file.path),
        ]
        .iter()
        .any(|path| {
            path.metadata()
                .is_ok_and(|metadata| metadata.len() == file.bytes)
        });
        if !present {
            missing += file.bytes;
        }
        let _ = writeln!(
            text,
            "  {} · {} bytes · {}",
            file.path,
            file.bytes,
            if present {
                "present; verified on acquire"
            } else {
                "to download"
            }
        );
    }
    let _ = write!(
        text,
        "{:.2} GiB to download · disk {}\nno repository code runs; files are verified against pinned SHA-256 hashes\n",
        missing as f64 / (1024.0 * 1024.0 * 1024.0),
        disk_free_bytes.map_or_else(
            || "free space unknown".into(),
            |bytes| format!("{:.1} GiB free", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        )
    );
    if disk_free_bytes.is_some_and(|free| free < missing) {
        text.push_str("blocked: not enough free disk space\n");
    }
    text
}

/// Downloads, verifies and promotes every file of `entry` into `destination/ID`.
/// `progress` receives a short status after each file.
///
/// # Errors
/// Fails on a network error, a size or hash mismatch, cancellation, or an existing destination
/// whose contents do not verify; verified staged files are kept for a retry.
pub fn acquire(
    entry: &CatalogEntry,
    destination: &Path,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(&str),
) -> io::Result<PathBuf> {
    entry.validate()?;
    let target = directory(destination, entry);
    refuse_link(&target)?;
    if target.exists() {
        verify_all(entry, &target)?;
        return Ok(target);
    }
    let staged = staging(destination, entry);
    refuse_link(&staged)?;
    fs::create_dir_all(&staged)?;
    // Redirects are followed by hand so each target can be checked; no proxy is taken from
    // the environment.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(None)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .user_agent("sefai-setup")
        .build()
        .into();
    for (index, file) in entry.files.iter().enumerate() {
        let path = staged.join(&file.path);
        if verify(&path, file.bytes, &file.sha256).is_ok() {
            continue;
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "acquisition cancelled; verified files stay staged",
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        download(&agent, entry, &file.path, file.bytes, &path, cancelled)?;
        verify(&path, file.bytes, &file.sha256)?;
        progress(&format!(
            "verified {}/{} · {}",
            index + 1,
            entry.files.len(),
            file.path
        ));
    }
    fs::rename(&staged, &target)?;
    Ok(target)
}

fn download(
    agent: &ureq::Agent,
    entry: &CatalogEntry,
    name: &str,
    bytes: u64,
    path: &Path,
    cancelled: &AtomicBool,
) -> io::Result<()> {
    let mut url = format!(
        "https://{HOST}/{}/resolve/{}/{name}",
        entry.repository, entry.revision
    );
    let mut redirects = 0;
    let response = loop {
        let response = agent
            .get(&url)
            .config()
            .timeout_recv_body(Some(body_deadline(bytes)))
            .build()
            .call()
            .map_err(|error| io::Error::other(format!("downloading {name}: {error}")))?;
        let status = response.status();
        if status.is_redirection() {
            if redirects == REDIRECTS_MAX {
                return Err(io::Error::other(format!("{name}: too many redirects")));
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| io::Error::other(format!("{name}: redirect without a location")))?;
            url = redirect_target(&url, location)?;
            redirects += 1;
            continue;
        }
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "downloading {name}: HTTP {status}"
            )));
        }
        break response;
    };
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    // A leftover partial file, or a link planted in its place, is removed rather than followed.
    if fs::symlink_metadata(&partial).is_ok() {
        fs::remove_file(&partial)?;
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)?;
    let mut body = response.into_body().into_reader().take(bytes + 1);
    let mut buffer = vec![0; CHUNK_BYTES];
    let mut written = 0u64;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            drop(output);
            let _ = fs::remove_file(&partial);
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "acquisition cancelled; verified files stay staged",
            ));
        }
        let count = body.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        written += count as u64;
        if written > bytes {
            drop(output);
            let _ = fs::remove_file(&partial);
            return Err(io::Error::other(format!(
                "{name}: larger than its pinned size"
            )));
        }
        output.write_all(&buffer[..count])?;
    }
    output.sync_all()?;
    drop(output);
    fs::rename(partial, path)
}

fn body_deadline(bytes: u64) -> Duration {
    Duration::from_secs(60 + bytes / BODY_BYTES_PER_SECOND_MIN)
}

/// Resolves a redirect and accepts only HTTPS on Hugging Face's own hosts.
fn redirect_target(current: &str, location: &str) -> io::Result<String> {
    let target = if location.starts_with('/') && !location.starts_with("//") {
        let origin_end = current
            .find("://")
            .and_then(|scheme| {
                current[scheme + 3..]
                    .find('/')
                    .map(|path| scheme + 3 + path)
            })
            .unwrap_or(current.len());
        format!("{}{location}", &current[..origin_end])
    } else {
        location.to_owned()
    };
    let uri: ureq::http::Uri = target
        .parse()
        .map_err(|_| io::Error::other("redirect target is not a valid URL"))?;
    let host = uri.host().unwrap_or_default().to_ascii_lowercase();
    let allowed = ["huggingface.co", "hf.co"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
    if uri.scheme_str() != Some("https") || !allowed {
        return Err(io::Error::other(format!(
            "redirect to {host} refused; downloads stay on Hugging Face hosts over https"
        )));
    }
    Ok(target)
}

/// Staging and destination directories must not be symbolic links planted elsewhere.
fn refuse_link(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::other(format!(
            "{} is a symbolic link; refusing to write through it",
            path.display()
        )));
    }
    Ok(())
}

fn verify_all(entry: &CatalogEntry, directory: &Path) -> io::Result<()> {
    for file in &entry.files {
        verify(&directory.join(&file.path), file.bytes, &file.sha256)?;
    }
    Ok(())
}

fn verify(path: &Path, bytes: u64, sha256: &str) -> io::Result<()> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() != bytes {
        return Err(io::Error::other(format!(
            "{}: size mismatch",
            path.display()
        )));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; CHUNK_BYTES];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let digest = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        });
    if digest != sha256 {
        return Err(io::Error::other(format!(
            "{}: sha256 mismatch",
            path.display()
        )));
    }
    Ok(())
}

/// The Markdown profile that selects an acquired entry; the path is workspace-relative when
/// the bundle lives inside the workspace.
pub fn profile(entry: &CatalogEntry, path: &Path) -> String {
    let path = path.display().to_string().replace('\'', "''");
    format!(
        "---\nschema: 1\nid: {}\npath: '{path}'\nformat: {}\n---\nAcquired from {}@{} by `sefai setup`.\n",
        entry.id,
        entry.format.name(),
        entry.repository,
        entry.revision
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_acquisition_publishes_nothing() {
        let entries = super::super::catalog::entries(None).expect("built-in catalog");
        let entry = entries.first().expect("an entry");
        let destination =
            std::env::temp_dir().join(format!("sefai-acquire-cancel-{}", std::process::id()));
        let result = acquire(entry, &destination, &AtomicBool::new(true), |_| {});
        let published = directory(&destination, entry).exists();
        let _ = fs::remove_dir_all(&destination);
        assert!(result.is_err_and(|error| error.kind() == io::ErrorKind::Interrupted));
        assert!(!published);
    }

    #[test]
    fn verification_rejects_a_changed_byte() {
        let path = std::env::temp_dir().join(format!("sefai-acquire-{}", std::process::id()));
        fs::write(&path, b"abc").expect("fixture");
        let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify(&path, 3, good).is_ok());
        fs::write(&path, b"abd").expect("fixture");
        assert!(verify(&path, 3, good).is_err());
        fs::remove_file(&path).expect("remove fixture");
    }

    #[test]
    fn redirects_stay_on_hugging_face_hosts_over_https() {
        let current = "https://huggingface.co/owner/name/resolve/0123/model.gguf";
        assert_eq!(
            redirect_target(current, "/api/resolve-cache/x").expect("relative"),
            "https://huggingface.co/api/resolve-cache/x"
        );
        assert!(redirect_target(current, "https://cas-bridge.xethub.hf.co/x").is_ok());
        assert!(redirect_target(current, "https://cdn-lfs.huggingface.co/x").is_ok());
        for refused in [
            "http://cdn-lfs.huggingface.co/x",
            "https://evil.example/x",
            "https://huggingface.co.evil.example/x",
            "https://169.254.169.254/latest",
            "//evil.example/x",
        ] {
            assert!(redirect_target(current, refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn body_deadlines_grow_with_file_size() {
        assert_eq!(body_deadline(0), Duration::from_secs(60));
        assert_eq!(
            body_deadline(1024 * 1024 * 1024),
            Duration::from_secs(60 + 4096)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_staging_directory_is_refused() {
        let entries = super::super::catalog::entries(None).expect("built-in catalog");
        let entry = entries.first().expect("an entry");
        let destination =
            std::env::temp_dir().join(format!("sefai-acquire-link-{}", std::process::id()));
        fs::create_dir_all(&destination).expect("destination");
        let elsewhere = destination.join("elsewhere");
        fs::create_dir_all(&elsewhere).expect("elsewhere");
        std::os::unix::fs::symlink(&elsewhere, staging(&destination, entry)).expect("link");
        let result = acquire(entry, &destination, &AtomicBool::new(false), |_| {});
        let _ = fs::remove_dir_all(&destination);
        assert!(result.is_err_and(|error| error.to_string().contains("symbolic link")));
    }
}
