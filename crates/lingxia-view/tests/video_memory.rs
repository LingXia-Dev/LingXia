//! The Apple video frame pull must hold Metal memory and IOSurfaces flat while a clip loops.
//! No harness: AVPlayer's clock advances only while the main run loop turns, as it does in the
//! app, and only `main` runs on the main thread. Opt-in, a minute on a GPU:
//! `LXV_VIDEO_MEMORY=1 cargo test -p lingxia-view --features gpu --test video_memory`;
//! `LXV_SECONDS` sets the run (default 60).

#[cfg(all(feature = "gpu", target_os = "macos"))]
fn main() {
    measure::run();
}

#[cfg(not(all(feature = "gpu", target_os = "macos")))]
fn main() {}

#[cfg(all(feature = "gpu", target_os = "macos"))]
mod measure {
    use lingxia_view::renderer::gpu::video::measure_player;
    use lingxia_view::renderer::gpu::wgpu;
    use lingxia_view::scene::video::Settings;
    use std::ffi::{c_char, c_void};
    use std::time::{Duration, Instant};

    #[link(name = "CoreFoundation", kind = "framework")]
    #[link(name = "objc")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, once: bool) -> i32;
        fn objc_msgSend();
        fn sel_registerName(name: *const c_char) -> *const c_void;
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    }

    fn footprint_mib() -> f64 {
        let mut info = [0u64; 64];
        let mut count = 128u32;
        unsafe { task_info(mach_task_self_, 22, info.as_mut_ptr().cast(), &mut count) };
        info[18] as f64 / 1_048_576.0
    }

    fn metal_mib(device: &wgpu::Device) -> f64 {
        unsafe {
            let hal = device.as_hal::<wgpu::hal::api::Metal>().unwrap();
            let raw = &**hal.raw_device() as *const _ as *const c_void;
            let send: unsafe extern "C" fn(*const c_void, *const c_void) -> usize =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            send(raw, sel_registerName(c"currentAllocatedSize".as_ptr())) as f64 / 1_048_576.0
        }
    }

    /// IOSurface regions mapped into this process, as vmmap counts them.
    fn iosurfaces() -> f64 {
        std::process::Command::new("vmmap")
            .args(["-summary", &std::process::id().to_string()])
            .output()
            .ok()
            .and_then(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .find(|line| line.starts_with("IOSurface "))
                    .and_then(|line| line.split_whitespace().last()?.parse().ok())
            })
            .unwrap_or(-1.0)
    }

    /// Least-squares slope of `(t, y)`.
    fn slope(points: &[(f64, f64)]) -> f64 {
        let n = points.len() as f64;
        let (mt, my) = points
            .iter()
            .fold((0.0, 0.0), |(a, b), (t, y)| (a + t / n, b + y / n));
        let num: f64 = points.iter().map(|(t, y)| (t - mt) * (y - my)).sum();
        let den: f64 = points.iter().map(|(t, _)| (t - mt).powi(2)).sum();
        num / den
    }

    fn env(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    pub fn run() {
        if std::env::var("LXV_VIDEO_MEMORY").is_err() {
            eprintln!("video_memory: skipped; set LXV_VIDEO_MEMORY=1 to run it");
            return;
        }
        let seconds = env("LXV_SECONDS", 60);
        let instance = wgpu::Instance::default();
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            eprintln!("video_memory: no GPU adapter, skipped");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let clip = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/lingxia-showcase-gpu/lxapp/public/video/q360.mp4"
        );
        let mut player = measure_player(&format!("file://{clip}")).unwrap();
        player.apply(
            &Settings {
                muted: true,
                looped: true,
                ..Settings::default()
            },
            None,
        );
        let start = Instant::now();
        let (mut shown, mut frames, mut import_us) = (None, 0u64, Vec::new());
        let (mut samples, mut next) = (Vec::new(), Duration::ZERO);
        while start.elapsed() < Duration::from_secs(seconds) {
            if player.has_new() {
                let at = Instant::now();
                if let Some((texture, _)) = player.next_frame(&device, &queue).unwrap() {
                    import_us.push(at.elapsed().as_micros() as u64);
                    // The renderer keeps the newest frame and drops the one before.
                    shown = Some(texture);
                    frames += 1;
                }
            }
            let _ = device.poll(wgpu::PollType::Poll);
            if start.elapsed() >= next {
                let t = start.elapsed().as_secs_f64();
                let (metal, foot, surfaces) = (metal_mib(&device), footprint_mib(), iosurfaces());
                eprintln!(
                    "t={t:5.1}s frames={frames:5} clock={:6.2}s metal {metal:7.2} MiB footprint {foot:7.1} MiB iosurfaces {surfaces}",
                    player.status().current_time
                );
                samples.push((t, metal, foot, surfaces));
                next += Duration::from_secs(5);
            }
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.016, false) };
        }
        drop(shown);
        drop(player);
        import_us.sort_unstable();
        let pick = |q: f64| import_us[((import_us.len() - 1) as f64 * q) as usize];
        // From 10 s on: the decoder's and the cache's pools have filled.
        let settled: Vec<_> = samples.iter().filter(|s| s.0 >= 10.0).collect();
        let per = |f: fn(&(f64, f64, f64, f64)) -> f64| {
            slope(&settled.iter().map(|s| (s.0, f(s))).collect::<Vec<_>>())
        };
        let (metal, foot, surfaces) = (per(|s| s.1), per(|s| s.2), per(|s| s.3));
        eprintln!(
            "frames {frames}; import p50 {}us p99 {}us max {}us; slope metal {metal:.4} MiB/s footprint {foot:.4} MiB/s iosurfaces {surfaces:.4}/s",
            pick(0.5),
            pick(0.99),
            import_us[import_us.len() - 1]
        );
        // The loop takes about half of a 30 fps clip's frames; fewer means it is not playing.
        assert!(
            frames as f64 > seconds as f64 * 8.0,
            "the clip did not play: {frames} frames in {seconds} s"
        );
        assert!(metal < 0.02, "Metal grows {metal:.4} MiB/s");
        assert!(surfaces < 0.05, "IOSurfaces grow {surfaces:.4}/s");
        eprintln!("video_memory: ok");
    }
}
