//! Watch the viewer behave, and check it against the real window.
//!
//! This macOS inspection harness watches the AppKit panel. The viewer has no
//! command surface: nothing opens or
//! closes it, and the host only says what just happened to the machine. So
//! everything here drives it the way a real run does — through
//! `supervision::note_activity` — and every assertion reads the actual window from the
//! window server rather than anything this process believes.
//!
//!     cargo run --release -p lingxia-device-io --features supervision --example pip_viewer
//!     cargo run --release -p lingxia-device-io --features supervision --example pip_viewer -- circle 120

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("this visual inspection harness is macOS-only; the viewer also runs on Windows");
}

#[cfg(target_os = "macos")]
fn main() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "behaviours".into());
    let hold: u64 = std::env::args()
        .nth(2)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(60);

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    // Accessory: a panel with no dock icon and no menu bar, which is what this
    // looks like inside a real product too.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    std::thread::spawn(move || {
        match mode.as_str() {
            "circle" => circle(hold),
            _ => behaviours(),
        }
        std::process::exit(0);
    });

    app.run();
}

/// Where the panel is, straight from the window server, or `None` when it is
/// not on screen. This deliberately bypasses `lingxia_device_io::windows()`:
/// the public device surface must exclude the viewer from its own targets.
#[cfg(target_os = "macos")]
fn panel() -> Option<(i32, i32)> {
    use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFString, CGRect};
    use objc2_core_graphics::{
        CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption,
        kCGWindowBounds, kCGWindowLayer, kCGWindowOwnerPID,
    };
    use std::ffi::c_void;

    unsafe extern "C-unwind" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFNumberGetTypeID() -> usize;
    }

    unsafe fn number(dict: *const c_void, key: &CFString) -> Option<i64> {
        let value = unsafe { CFDictionaryGetValue(dict, key as *const CFString as *const c_void) };
        if value.is_null() || unsafe { CFGetTypeID(value) != CFNumberGetTypeID() } {
            return None;
        }
        unsafe { (*(value as *const CFNumber)).as_i64() }
    }

    let info = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        0,
    )?;
    let array = (&*info as *const CFArray).cast::<c_void>();
    let pid = std::process::id() as i64;

    unsafe {
        for index in 0..CFArrayGetCount(array).max(0) {
            let dict = CFArrayGetValueAtIndex(array, index);
            if dict.is_null()
                || number(dict, kCGWindowOwnerPID) != Some(pid)
                || number(dict, kCGWindowLayer).unwrap_or(0) <= 0
            {
                continue;
            }
            let value =
                CFDictionaryGetValue(dict, kCGWindowBounds as *const CFString as *const c_void);
            if value.is_null() {
                continue;
            }
            let mut bounds = CGRect::default();
            if CGRectMakeWithDictionaryRepresentation(
                Some(&*(value as *const CFDictionary)),
                &mut bounds,
            ) {
                return Some((
                    bounds.origin.x.round() as i32,
                    bounds.origin.y.round() as i32,
                ));
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn centre() -> (i32, i32) {
    use lingxia_device_io as cu;
    let b = cu::displays().expect("displays")[0].bounds;
    (b.x + b.w / 2, b.y + b.h / 2)
}

#[cfg(target_os = "macos")]
fn circle(hold: u64) {
    use lingxia_device_io as cu;
    use std::time::Duration;

    let bounds = cu::displays().expect("displays")[0].bounds;
    let (cx, cy) = centre();
    let radius = (bounds.w.min(bounds.h) / 3) as f64;

    println!("driving for {hold}s — nothing calls show; the viewer opens itself");
    for step in 0..(hold * 1000 / 120) {
        let angle = step as f64 * 0.18;
        cu::supervision::note_activity(cu::Acted::At {
            x: cx + (radius * angle.cos()) as i32,
            y: cy + (radius * angle.sin()) as i32,
        });
        std::thread::sleep(Duration::from_millis(120));
    }
}

/// The four behaviours that exist because this watches live work.
#[cfg(target_os = "macos")]
fn behaviours() {
    use lingxia_device_io as cu;
    use std::time::Duration;

    let (cx, cy) = centre();
    assert!(panel().is_none(), "nothing should be on screen yet");

    println!("\n1. opens by itself on the first thing that changes the machine");
    cu::supervision::note_activity(cu::Acted::At { x: cx, y: cy });
    std::thread::sleep(Duration::from_secs(2));
    let opened = panel().expect("the first actuating command must open it");
    println!("   panel at {opened:?}, with nobody having asked for it");
    let own_windows = cu::windows(&cu::WindowQuery::parse(&format!(
        "pid:{}",
        std::process::id()
    )))
    .expect("public window enumeration");
    assert!(
        own_windows.is_empty(),
        "the activity viewer must not be exposed as a device target"
    );

    println!("\n2. moves aside when the work is underneath it");
    for _ in 0..12 {
        cu::supervision::note_activity(cu::Acted::At {
            x: opened.0 + 60,
            y: opened.1 + 60,
        });
        std::thread::sleep(Duration::from_millis(150));
    }
    std::thread::sleep(Duration::from_secs(1));
    let moved = panel().expect("still on screen");
    println!("   panel now at {moved:?}");
    assert_ne!(
        opened, moved,
        "a viewer sitting on the work must take itself out of the way"
    );

    println!("\n3. leaves when the work stops (idle, ~12s)");
    let mut rested = None;
    for second in 1..=18 {
        std::thread::sleep(Duration::from_secs(1));
        if panel().is_none() {
            rested = Some(second);
            break;
        }
    }
    let rested = rested.expect("an idle viewer must put itself away");
    println!("   gone after {rested}s, with nobody having closed it");

    println!("\n4. comes straight back on the next thing that happens");
    cu::supervision::note_activity(cu::Acted::At {
        x: cx / 2,
        y: cy / 2,
    });
    std::thread::sleep(Duration::from_secs(2));
    let back = panel().expect("the next action must bring it back");
    println!("   panel at {back:?}");

    println!("\nall four behaved as described, and no command was involved");
}
