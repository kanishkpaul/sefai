//! `sefai setup` text reports; the same text backs the terminal's `/setup` overlay.

use super::{
    Artifact, catalog,
    estimate::estimate,
    hardware::{Availability, Hardware},
    inspect, snapshot,
};
use crate::workspace::{Format, Purpose, Selection, Workspace};
use clap::Subcommand;
use std::{fmt::Write, io, path::PathBuf};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

#[derive(Subcommand)]
pub enum Action {
    /// Show memory, CPU, workspace disk and the engines this build can run.
    Hardware,
    /// Estimate a local model's memory from its metadata without loading it.
    Check {
        path: PathBuf,
        #[arg(long, value_enum)]
        format: Format,
        #[arg(long, default_value_t = 4096)]
        context: u32,
    },
    /// Shortlist catalog models that fit this machine for a purpose.
    Recommend {
        #[arg(long, value_enum, default_value_t = Purpose::Chat)]
        purpose: Purpose,
        #[arg(long, default_value_t = 4096)]
        context: u32,
    },
    /// List every catalog entry and what blocks it here.
    Catalog,
    /// Preview a catalog entry's download; `--yes` downloads, verifies and writes its profile.
    Get {
        id: String,
        /// Parent directory for the model; defaults to the workspace's `weights` directory.
        #[arg(long)]
        to: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
    },
    /// Write a profile for a local model after checking its metadata; no files are copied.
    Import {
        id: String,
        path: PathBuf,
        #[arg(long, value_enum)]
        format: Format,
    },
}

/// Runs one setup action and returns its report; `cancelled` stops a download between reads.
///
/// # Errors
/// Fails for an unreadable workspace catalog or artifact metadata.
pub fn report(
    selection: &Selection,
    action: &Action,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<String> {
    let root = selection.root()?;
    let workspace = root.exists().then(|| Workspace::open(&root)).transpose()?;
    let hardware = snapshot(&root);
    match action {
        Action::Hardware => Ok(hardware_report(&hardware)),
        Action::Check {
            path,
            format,
            context,
        } => check_report(&inspect(path, *format)?, &hardware, *context),
        Action::Recommend { purpose, context } => {
            let entries = catalog::entries(workspace.as_ref())?;
            let choices = catalog::recommend(&entries, &hardware, *purpose, *context);
            let mut text = format!(
                "{} candidates for {} at context {context} · available {:.1} GiB\n",
                choices.len(),
                purpose_name(*purpose),
                hardware.memory_available_bytes as f64 / GIB
            );
            if choices.is_empty() {
                text.push_str(
                    "no catalog entry runs and fits here; see `sefai setup catalog` for blockers\n",
                );
            }
            for choice in choices {
                let _ = writeln!(
                    text,
                    "{} · {} · {} · {:.2}–{:.2} GiB estimated · {} · {}",
                    choice.entry.id,
                    choice.entry.format.name(),
                    choice.reason,
                    choice.estimate.low_bytes as f64 / GIB,
                    choice.estimate.high_bytes as f64 / GIB,
                    choice.fit.name(),
                    choice.entry.license
                );
            }
            text.push_str("estimates use Phase 10 measurements from one Apple M5; nothing was downloaded or loaded\n");
            Ok(text)
        }
        Action::Get { id, to, yes } => {
            let entries = catalog::entries(workspace.as_ref())?;
            let entry = entries
                .iter()
                .find(|entry| &entry.id == id)
                .ok_or_else(|| invalid(format!("catalog has no entry {id}")))?;
            if let Some(reason) = catalog::blocker(entry, &hardware) {
                return Err(invalid(format!("{id} cannot run here: {reason}")));
            }
            let destination = to.clone().unwrap_or_else(|| root.join("weights"));
            get(
                entry,
                &destination,
                &hardware,
                *yes,
                workspace.as_ref(),
                cancelled,
            )
        }
        Action::Import { id, path, format } => {
            let workspace =
                workspace.ok_or_else(|| invalid("initialize the workspace before importing"))?;
            let artifact = inspect(path, *format)?;
            let path = path.canonicalize()?;
            let text = format!(
                "---\nschema: 1\nid: {id}\npath: '{}'\nformat: {}\n---\nImported with `sefai setup import`.\n",
                path.display().to_string().replace('\'', "''"),
                format.name()
            );
            workspace.put(
                std::path::Path::new(&format!("models/{id}.md")),
                None,
                text.as_bytes(),
            )?;
            Ok(format!(
                "wrote models/{id}.md for {} ({} layers); select it with /profile {id} or --profile {id}\n",
                artifact.path.display(),
                artifact.shape.layers
            ))
        }
        Action::Catalog => {
            let entries = catalog::entries(workspace.as_ref())?;
            let mut text = String::new();
            for entry in &entries {
                let download = entry.weight_bytes() as f64 / GIB;
                let state = catalog::blocker(entry, &hardware).unwrap_or_else(|| "runnable".into());
                let _ = writeln!(
                    text,
                    "{} · {} · {}M parameters · {:.2} GiB download · {} · {}@{} · evidence {} · {}",
                    entry.id,
                    entry.format.name(),
                    entry.parameters_millions,
                    download,
                    entry
                        .purposes
                        .iter()
                        .map(|purpose| purpose_name(*purpose))
                        .collect::<Vec<_>>()
                        .join("+"),
                    entry.repository,
                    &entry.revision[..12],
                    entry.evidence,
                    state
                );
            }
            Ok(text)
        }
    }
}

#[cfg(feature = "download")]
fn get(
    entry: &crate::workspace::CatalogEntry,
    destination: &std::path::Path,
    hardware: &Hardware,
    confirmed: bool,
    workspace: Option<&Workspace>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<String> {
    let mut text = super::acquire::preview(entry, destination, hardware.disk_free_bytes);
    if !confirmed {
        text.push_str("nothing was downloaded; confirm with --yes, or /setup get ID confirm in the terminal\n");
        return Ok(text);
    }
    if text.contains("blocked:") {
        return Err(invalid("not enough free disk space for this download"));
    }
    let workspace =
        workspace.ok_or_else(|| invalid("initialize the workspace before acquiring a model"))?;
    let directory = super::acquire::acquire(entry, destination, cancelled, |status| {
        text.push_str(status);
        text.push('\n');
    })?;
    let model = if entry.entry == "." {
        directory
    } else {
        directory.join(&entry.entry)
    };
    // A model inside the workspace is recorded relative to it, so the workspace can move.
    let root = workspace.root().canonicalize()?;
    let model = model
        .canonicalize()?
        .strip_prefix(&root)
        .map(std::path::Path::to_path_buf)
        .unwrap_or(model);
    let profile = std::path::PathBuf::from(format!("models/{}.md", entry.id));
    let profile_text = super::acquire::profile(entry, &model);
    if !workspace.root().join(&profile).exists() {
        workspace.put(&profile, None, profile_text.as_bytes())?;
    } else if workspace.read(&profile)? != profile_text {
        return Err(invalid(format!(
            "{} exists with other settings; edit it or remove it, then retry",
            profile.display()
        )));
    }
    let _ = writeln!(
        text,
        "{} is ready; select it with /profile {} or --profile {}",
        profile.display(),
        entry.id,
        entry.id
    );
    Ok(text)
}

#[cfg(not(feature = "download"))]
fn get(
    entry: &crate::workspace::CatalogEntry,
    _destination: &std::path::Path,
    _hardware: &Hardware,
    _confirmed: bool,
    _workspace: Option<&Workspace>,
    _cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<String> {
    Ok(format!(
        "this build cannot download; rebuild with the download feature, or fetch {}@{} yourself and run `sefai setup import {} PATH --format {}`\n",
        entry.repository,
        entry.revision,
        entry.id,
        entry.format.name()
    ))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn purpose_name(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::Chat => "chat",
        Purpose::Tools => "tools",
    }
}

fn hardware_report(hardware: &Hardware) -> String {
    let mut text = format!(
        "{} · {} · {} · {} threads\nmemory {:.1} GiB total, {:.1} GiB available{}\n",
        hardware.os,
        hardware.architecture,
        hardware.cpu,
        hardware.cores,
        hardware.memory_total_bytes as f64 / GIB,
        hardware.memory_available_bytes as f64 / GIB,
        if hardware.unified_memory {
            " · unified with the GPU"
        } else {
            ""
        }
    );
    match hardware.disk_free_bytes {
        Some(bytes) => {
            let _ = writeln!(text, "workspace disk {:.1} GiB free", bytes as f64 / GIB);
        }
        None => text.push_str("workspace disk free space unknown\n"),
    }
    for engine in &hardware.engines {
        let state = match &engine.state {
            Availability::Compiled => "compiled; a load still has to succeed".to_owned(),
            Availability::MissingFeature(feature) => {
                format!("not in this build; needs the {feature} feature")
            }
            Availability::Unsupported => "unsupported on this platform".to_owned(),
        };
        let _ = writeln!(text, "{} · {state}", engine.name);
    }
    text.push_str("GPU memory budgets are not probed; CUDA and Vulkan routes are compiled-only in Phase 10 evidence\n");
    text
}

fn check_report(artifact: &Artifact, hardware: &Hardware, context: u32) -> io::Result<String> {
    let shape = &artifact.shape;
    if let Some(trained) = shape.context
        && u64::from(context) > trained
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("context {context} exceeds the model's trained context {trained}"),
        ));
    }
    let estimate = estimate(
        artifact.format,
        artifact.weight_bytes,
        shape.layers,
        shape.kv_heads,
        shape.head_dim,
        artifact.cache_element_bytes,
        u64::from(context),
    );
    Ok(format!(
        "{} · {} · {} layers · {} KV heads × {} dimensions · trained context {}\nweights {:.2} GiB · KV cache {:.2} GiB at {context} tokens ({} bytes per element) · overhead {:.2} GiB\nestimated {:.2}–{:.2} GiB · {} against {:.1} GiB available\nmetadata only: no weights were loaded; measure with `sefai bench` before relying on it\n",
        artifact.path.display(),
        artifact.format.name(),
        shape.layers,
        shape.kv_heads,
        shape.head_dim,
        shape
            .context
            .map_or_else(|| "unknown".into(), |value| value.to_string()),
        estimate.weight_bytes as f64 / GIB,
        estimate.cache_bytes as f64 / GIB,
        artifact.cache_element_bytes,
        estimate.overhead_bytes as f64 / GIB,
        estimate.low_bytes as f64 / GIB,
        estimate.high_bytes as f64 / GIB,
        estimate.fit(hardware.memory_available_bytes).name(),
        hardware.memory_available_bytes as f64 / GIB,
    ))
}
