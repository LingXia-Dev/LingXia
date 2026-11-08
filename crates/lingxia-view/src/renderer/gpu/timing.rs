//! GPU time of sampled redraws, from timestamp queries around each pass they submit. Only
//! the GPU's busy spans count: the gaps while the CPU encodes the next pass do not.
use std::sync::{Arc, Mutex};

/// Timestamp pairs one sampled redraw may take.
const PAIRS: u32 = 16;

struct Sample {
    set: wgpu::QuerySet,
    used: u32,
    /// A 1×1 target: Metal drops an empty pass, and its timestamps with it.
    probe: wgpu::TextureView,
}

static ACTIVE: Mutex<Option<Sample>> = Mutex::new(None);
/// Sampled redraws: count, total and max µs.
static GPU_US: Mutex<[u64; 3]> = Mutex::new([0; 3]);

/// Starts measuring a redraw; false when the device has no timestamps.
pub(crate) fn start(device: &wgpu::Device) -> bool {
    if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        return false;
    }
    let set = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("LingXia redraw timing"),
        ty: wgpu::QueryType::Timestamp,
        count: PAIRS * 2,
    });
    let probe = device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    *ACTIVE.lock().unwrap_or_else(|p| p.into_inner()) = Some(Sample {
        set,
        used: 0,
        probe,
    });
    true
}

/// Opens a timed span in `encoder` while a redraw is measured; pass it to [`end`].
pub(crate) fn begin(encoder: &mut wgpu::CommandEncoder) -> Option<u32> {
    let mut active = ACTIVE.lock().unwrap_or_else(|p| p.into_inner());
    let sample = active.as_mut().filter(|sample| sample.used < PAIRS)?;
    let index = sample.used * 2;
    sample.used += 1;
    stamp(encoder, sample, Some(index), None);
    Some(index)
}

pub(crate) fn end(encoder: &mut wgpu::CommandEncoder, span: Option<u32>) {
    let Some(index) = span else {
        return;
    };
    let active = ACTIVE.lock().unwrap_or_else(|p| p.into_inner());
    let Some(sample) = active.as_ref() else {
        return;
    };
    stamp(encoder, sample, None, Some(index + 1));
}

fn stamp(
    encoder: &mut wgpu::CommandEncoder,
    sample: &Sample,
    beginning: Option<u32>,
    end: Option<u32>,
) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &sample.probe,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Discard,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
            query_set: &sample.set,
            beginning_of_pass_write_index: beginning,
            end_of_pass_write_index: end,
        }),
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

/// Ends the measured redraw: its spans are read back off the render thread.
pub(crate) fn finish(device: &wgpu::Device, queue: &wgpu::Queue) {
    let Some(sample) = ACTIVE.lock().unwrap_or_else(|p| p.into_inner()).take() else {
        return;
    };
    if sample.used == 0 {
        return;
    }
    let bytes = u64::from(sample.used) * 2 * 8;
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }));
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.resolve_query_set(&sample.set, 0..sample.used * 2, &resolve, 0);
    encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, bytes);
    queue.submit([encoder.finish()]);
    let period = f64::from(queue.get_timestamp_period());
    let mapped = read.clone();
    read.slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if result.is_err() {
                return;
            }
            let Ok(range) = mapped.slice(..).get_mapped_range() else {
                return;
            };
            let ticks: Vec<u64> = range
                .chunks_exact(8)
                .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap_or_default()))
                .collect();
            drop(range);
            mapped.unmap();
            let ns: f64 = ticks
                .chunks_exact(2)
                .map(|pair| pair[1].saturating_sub(pair[0]) as f64 * period)
                .sum();
            let us = (ns / 1000.0) as u64;
            let mut gpu = GPU_US.lock().unwrap_or_else(|p| p.into_inner());
            *gpu = [gpu[0] + 1, gpu[1] + us, gpu[2].max(us)];
        });
    let device = device.clone();
    let _ = std::thread::Builder::new()
        .name("lingxia-gpu-timing".into())
        .spawn(move || {
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        });
}

/// GPU µs per sampled redraw: `{samples, avg, max}`.
pub(crate) fn stats() -> serde_json::Value {
    let [count, total, max] = *GPU_US.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::json!({"samples": count, "avg": total.checked_div(count), "max": max})
}
