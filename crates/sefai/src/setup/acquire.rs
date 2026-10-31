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
const READ_TIMEOUT: Duration = Duration::from_secs(60);
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
    if target.exists() {
        verify_all(entry, &target)?;
        return Ok(target);
    }
    let staged = staging(destination, entry);
    fs::create_dir_all(&staged)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(REDIRECTS_MAX)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_body(Some(READ_TIMEOUT))
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
    let url = format!(
        "https://{HOST}/{}/resolve/{}/{name}",
        entry.repository, entry.revision
    );
    let response = agent
        .get(&url)
        .call()
        .map_err(|error| io::Error::other(format!("downloading {name}: {error}")))?;
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let mut output = File::create(&partial)?;
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
}
