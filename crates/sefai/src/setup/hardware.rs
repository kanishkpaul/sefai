//! A one-shot hardware and runtime snapshot; nothing polls in the background.

use std::path::Path;

/// What setup can see about this machine, with absent facts left unknown.
#[derive(Clone, Debug)]
pub struct Hardware {
    pub os: String,
    pub architecture: &'static str,
    pub cpu: String,
    pub cores: usize,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    /// Apple Silicon shares one memory pool between CPU and GPU.
    pub unified_memory: bool,
    /// Free bytes on the disk holding the workspace, when it can be found.
    pub disk_free_bytes: Option<u64>,
    pub engines: Vec<Engine>,
}

/// One engine and whether this build can run it on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    pub name: &'static str,
    pub state: Availability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Compiled into this build for this platform; a load still has to succeed.
    Compiled,
    /// Not compiled; the named build feature adds it.
    MissingFeature(&'static str),
    /// This platform cannot run it.
    Unsupported,
}

/// Takes a snapshot of host memory, CPU, workspace disk and compiled engines.
pub fn snapshot(workspace: &Path) -> Hardware {
    use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
    // Each probe is optional: a platform that refuses one leaves that fact unknown.
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram())
            .with_cpu(CpuRefreshKind::nothing()),
    )
    .ok();
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_owned());
    // The disk whose mount point is the longest prefix of the workspace holds it.
    let disk_free_bytes = Disks::new_with_refreshed_list().ok().and_then(|disks| {
        disks
            .iter()
            .filter(|disk| workspace.starts_with(disk.mount_point()))
            .max_by_key(|disk| disk.mount_point().as_os_str().len())
            .map(|disk| disk.available_space())
    });
    let apple_silicon = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    Hardware {
        os: System::long_os_version().unwrap_or_else(|_| std::env::consts::OS.into()),
        architecture: std::env::consts::ARCH,
        cpu: system
            .as_ref()
            .and_then(|system| {
                system
                    .cpus()
                    .first()
                    .map(|cpu| cpu.brand().trim().to_owned())
            })
            .filter(|brand| !brand.is_empty())
            .unwrap_or_else(|| "unknown".into()),
        cores: std::thread::available_parallelism().map_or(1, |count| count.get()),
        memory_total_bytes: system.as_ref().map_or(0, System::total_memory),
        memory_available_bytes: system.as_ref().map_or(0, System::available_memory),
        unified_memory: apple_silicon,
        disk_free_bytes,
        engines: engines(apple_silicon),
    }
}

fn engines(apple_silicon: bool) -> Vec<Engine> {
    let state = |compiled: bool, feature: &'static str, supported: bool| {
        if !supported {
            Availability::Unsupported
        } else if compiled {
            Availability::Compiled
        } else {
            Availability::MissingFeature(feature)
        }
    };
    vec![
        Engine {
            name: "gguf cpu",
            state: state(cfg!(feature = "gguf"), "gguf", true),
        },
        Engine {
            name: "gguf metal",
            state: state(cfg!(feature = "metal"), "metal", cfg!(target_os = "macos")),
        },
        Engine {
            name: "gguf cuda",
            state: state(cfg!(feature = "cuda"), "cuda", !cfg!(target_os = "macos")),
        },
        Engine {
            name: "gguf vulkan",
            state: state(
                cfg!(feature = "vulkan"),
                "vulkan",
                !cfg!(target_os = "macos"),
            ),
        },
        Engine {
            name: "mlx, gptq and awq",
            state: state(cfg!(feature = "mlx"), "mlx", apple_silicon),
        },
        Engine {
            name: "onnx cpu",
            state: state(cfg!(feature = "onnx"), "onnx", true),
        },
        Engine {
            name: "onnx webgpu",
            state: state(cfg!(feature = "onnx-webgpu"), "onnx-webgpu", true),
        },
    ]
}
