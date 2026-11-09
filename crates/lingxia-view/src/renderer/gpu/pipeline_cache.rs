//! The driver's compiled render pipelines, kept on disk beside the View bytecode so a later
//! launch builds the renderer without compiling its shaders again.
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

pub(crate) struct PipelineCache {
    cache: wgpu::PipelineCache,
    path: PathBuf,
}

impl PipelineCache {
    /// None when the device cannot keep pipelines or no cache directory is set.
    pub(crate) fn open(device: &wgpu::Device, info: &wgpu::AdapterInfo) -> Option<Self> {
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            return None;
        }
        let path = crate::engine::bytecode::cache_dir()?.join(file_name(info)?);
        let data = std::fs::read(&path).ok();
        // SAFETY: the data is only ever what `get_data` wrote for this adapter and driver;
        // `fallback` starts empty when the driver rejects it.
        let cache = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("LingXia UI pipelines"),
                data: data.as_deref(),
                fallback: true,
            })
        };
        Some(Self { cache, path })
    }

    pub(crate) fn get(&self) -> &wgpu::PipelineCache {
        &self.cache
    }

    /// Writes what the driver compiled so far, off the render thread.
    pub(crate) fn save(&self) {
        let Some(data) = self.cache.get_data() else {
            return;
        };
        let path = self.path.clone();
        let spawned = std::thread::Builder::new()
            .name("view-pipelines".into())
            .spawn(move || {
                if let Err(error) = write(&path, &data) {
                    log::warn!(target: "lingxia_view", "PIPELINE_CACHE: cannot write cache: {error}");
                }
            });
        if let Err(error) = spawned {
            log::warn!(target: "lingxia_view", "PIPELINE_CACHE: cannot write cache: {error}");
        }
    }
}

/// wgpu's key names the vendor and device; the driver build and this crate's version are
/// added so an update never feeds the driver another build's binaries.
fn file_name(info: &wgpu::AdapterInfo) -> Option<String> {
    let base = wgpu::util::pipeline_cache_key(info)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (&info.driver, &info.driver_info, env!("CARGO_PKG_VERSION")).hash(&mut hasher);
    Some(format!("{base}_{:016x}.bin", hasher.finish()))
}

/// Replaces the file whole, and drops caches left by other drivers or versions.
fn write(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(dir)?;
    let staging = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&staging, data)?;
    std::fs::rename(&staging, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&staging);
    })?;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let other = entry.path();
        let stale = other != path
            && other
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("wgpu_pipeline_cache_") && name.ends_with(".bin")
                });
        if stale {
            let _ = std::fs::remove_file(other);
        }
    }
    Ok(())
}
