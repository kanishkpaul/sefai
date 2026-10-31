//! The offline catalog: built-in entries pinned to fixtures sefai's own tests run, plus any
//! entries in the workspace's `catalog` directory. Entries describe artifacts; they never
//! grant tool support, run code or change settings.

use super::{
    estimate::{Estimate, Fit, estimate},
    hardware::{Availability, Hardware},
};
use crate::workspace::{CatalogEntry, Format, Purpose, Workspace};
use std::io;

// Keep in sync with crates/sefai/catalog/.
const BUILT_IN: [&str; 14] = [
    include_str!("../../catalog/smollm2-135m-gguf.md"),
    include_str!("../../catalog/smollm2-1-7b-gguf.md"),
    include_str!("../../catalog/smollm2-135m-mlx.md"),
    include_str!("../../catalog/smollm2-1-7b-mlx.md"),
    include_str!("../../catalog/qwen3-0-6b-mlx.md"),
    include_str!("../../catalog/qwen3-0-6b-gptq.md"),
    include_str!("../../catalog/smollm2-1-7b-gptq.md"),
    include_str!("../../catalog/qwen3-0-6b-awq.md"),
    include_str!("../../catalog/smollm2-1-7b-awq.md"),
    include_str!("../../catalog/smollm2-135m-onnx.md"),
    include_str!("../../catalog/smollm2-1-7b-onnx.md"),
    include_str!("../../catalog/qwen3-0-6b-onnx.md"),
    include_str!("../../catalog/qwen3-0-6b-onnx-cpu-int4.md"),
    include_str!("../../catalog/qwen3-0-6b-onnx-webgpu-int4.md"),
];
const CATALOG_ENTRIES_MAX: usize = 128;
const CHOICES_MAX: usize = 3;

/// Built-in entries followed by the workspace's, with duplicate IDs rejected.
///
/// # Errors
/// Fails for an invalid workspace entry or one that reuses a built-in ID.
pub fn entries(workspace: Option<&Workspace>) -> io::Result<Vec<CatalogEntry>> {
    let mut entries = BUILT_IN
        .iter()
        .map(|text| {
            let (entry, _) = crate::workspace::parse_catalog(text)?;
            entry.validate()?;
            Ok(entry)
        })
        .collect::<io::Result<Vec<_>>>()?;
    if let Some(workspace) = workspace {
        for entry in workspace.catalog()? {
            if entries.iter().any(|existing| existing.id == entry.id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("catalog/{}.md: id duplicates a built-in entry", entry.id),
                ));
            }
            entries.push(entry);
        }
    }
    entries.truncate(CATALOG_ENTRIES_MAX);
    Ok(entries)
}

/// One explained candidate for the user's purpose and machine.
#[derive(Debug)]
pub struct Choice<'catalog> {
    pub entry: &'catalog CatalogEntry,
    pub estimate: Estimate,
    pub fit: Fit,
    pub reason: &'static str,
}

/// Why a catalog entry cannot run here, or `None` when it can be tried.
pub fn blocker(entry: &CatalogEntry, hardware: &Hardware) -> Option<String> {
    let engine = match entry.format {
        Format::Gguf => "gguf cpu",
        Format::Mlx | Format::Gptq | Format::Awq => "mlx, gptq and awq",
        Format::Onnx if entry.id.contains("webgpu") => "onnx webgpu",
        Format::Onnx => "onnx cpu",
    };
    match hardware
        .engines
        .iter()
        .find(|candidate| candidate.name == engine)
        .map(|candidate| &candidate.state)
    {
        Some(Availability::Compiled) => None,
        Some(Availability::MissingFeature(feature)) => {
            Some(format!("needs a build with the {feature} feature"))
        }
        Some(Availability::Unsupported) | None => {
            Some(format!("{engine} does not run on this platform"))
        }
    }
}

/// Up to three runnable entries for `purpose` at `context` tokens: the smallest that fits, a
/// balanced middle and the largest that still fits, without inventing a category that has no
/// distinct candidate.
pub fn recommend<'catalog>(
    entries: &'catalog [CatalogEntry],
    hardware: &Hardware,
    purpose: Purpose,
    context: u32,
) -> Vec<Choice<'catalog>> {
    let mut candidates = entries
        .iter()
        .filter(|entry| entry.purposes.contains(&purpose))
        .filter(|entry| context <= entry.trained_context)
        .filter(|entry| blocker(entry, hardware).is_none())
        .map(|entry| {
            let estimate = estimate(
                entry.format,
                entry.weight_bytes(),
                u64::from(entry.layers),
                u64::from(entry.kv_heads),
                u64::from(entry.head_dim),
                u64::from(entry.cache_element_bytes),
                u64::from(context),
            );
            let fit = estimate.fit(hardware.memory_available_bytes);
            (entry, estimate, fit)
        })
        .filter(|(_, _, fit)| matches!(fit, Fit::Comfortable | Fit::Tight))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(entry, estimate, _)| (estimate.high_bytes, entry.id.clone()));
    let mut choices = Vec::new();
    let picks = match candidates.len() {
        0 => vec![],
        1 => vec![(0, "only runnable candidate")],
        2 => vec![(0, "lower memory"), (1, "larger model that still fits")],
        count => vec![
            (0, "lower memory"),
            (count / 2, "balanced"),
            (count - 1, "largest that still fits"),
        ],
    };
    for (index, reason) in picks.into_iter().take(CHOICES_MAX) {
        let (entry, estimate, fit) = &candidates[index];
        choices.push(Choice {
            entry,
            estimate: estimate.clone(),
            fit: *fit,
            reason,
        });
    }
    choices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::hardware::Engine;

    fn hardware(available_gib: u64, engines: &[&'static str]) -> Hardware {
        Hardware {
            os: "test".into(),
            architecture: "test",
            cpu: "test".into(),
            cores: 1,
            memory_total_bytes: available_gib << 30,
            memory_available_bytes: available_gib << 30,
            unified_memory: false,
            disk_free_bytes: None,
            engines: engines
                .iter()
                .copied()
                .map(|name| Engine {
                    name,
                    state: Availability::Compiled,
                })
                .collect(),
        }
    }

    #[test]
    fn built_in_entries_parse_and_validate() {
        let entries = entries(None).expect("built-in catalog");
        assert_eq!(entries.len(), BUILT_IN.len());
    }

    #[test]
    fn recommendations_include_only_compiled_engines_that_fit() {
        let entries = entries(None).expect("built-in catalog");
        let choices = recommend(&entries, &hardware(8, &["gguf cpu"]), Purpose::Chat, 4096);
        assert!(!choices.is_empty() && choices.len() <= CHOICES_MAX);
        assert!(
            choices
                .iter()
                .all(|choice| choice.entry.format == Format::Gguf)
        );
    }

    #[test]
    fn nothing_is_recommended_without_enough_memory() {
        let entries = entries(None).expect("built-in catalog");
        assert!(recommend(&entries, &hardware(0, &["gguf cpu"]), Purpose::Tools, 4096).is_empty());
    }

    #[test]
    fn context_beyond_the_trained_limit_is_excluded() {
        let entries = entries(None).expect("built-in catalog");
        let choices = recommend(&entries, &hardware(64, &["gguf cpu"]), Purpose::Chat, 16384);
        assert!(
            choices
                .iter()
                .all(|choice| choice.entry.trained_context >= 16384)
        );
    }

    #[test]
    fn a_missing_feature_blocks_an_entry_with_its_name() {
        let entries = entries(None).expect("built-in catalog");
        let mut hardware = hardware(8, &[]);
        hardware.engines.push(Engine {
            name: "mlx, gptq and awq",
            state: Availability::MissingFeature("mlx"),
        });
        let gptq = entries
            .iter()
            .find(|entry| entry.format == Format::Gptq)
            .expect("gptq entry");
        assert!(blocker(gptq, &hardware).is_some_and(|reason| reason.contains("mlx feature")));
    }
}
