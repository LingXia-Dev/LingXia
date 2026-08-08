//! Puts the viewer on screen so it can be looked at, and photographs it.
//!
//! The viewer is an AppKit panel, so nothing about it can be judged from a unit
//! test: whether it appears, whether it lands in the right corner, whether it
//! leaves itself out of its own capture, whether the marker falls on the point
//! acted on, whether it moves aside when the work is underneath it, and whether
//! it goes away when the work stops are all questions only pixels answer.
//!
//!     cargo run --release -p lingxia-computer-use --example pip_viewer
//!     cargo run --release -p lingxia-computer-use --example pip_viewer -- circle 120
//!     cargo run --release -p lingxia-computer-use --example pip_viewer -- behaviours
//!
//! `circle` walks a marker around the screen so the viewer can be watched.
//! `behaviours` runs move-aside, idle-rest and wake in sequence, printing what
//! it expects before each one so a watcher can check it against the screen.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("the viewer is macOS-only");
}

#[cfg(target_os = "macos")]
fn main() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let mode = std::env::args().nth(1).unwrap_or_else(|| "circle".into());
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
            "behaviours" => behaviours(),
            _ => circle(hold),
        }
        std::process::exit(0);
    });

    app.run();
}

#[cfg(target_os = "macos")]
fn circle(hold: u64) {
    use lingxia_computer_use as cu;
    use std::time::Duration;

    std::thread::sleep(Duration::from_millis(600));
    let status = cu::pip::show(cu::PipWatch::Display(1), Some(cu::PipCorner::BottomRight))
        .expect("the viewer shows");
    println!("shown: {status:?}");

    let bounds = cu::displays().expect("displays")[0].bounds;
    let centre = (bounds.x + bounds.w / 2, bounds.y + bounds.h / 2);
    let radius = (bounds.w.min(bounds.h) / 3) as f64;

    // A marker that moves is the only way to tell a live viewer from a still
    // picture of one.
    println!("holding for {hold}s — the ring should be circling the screen");
    let steps = hold * 1000 / 120;
    for step in 0..steps {
        let angle = step as f64 * 0.18;
        cu::pip::note_activity(cu::Acted::At {
            x: centre.0 + (radius * angle.cos()) as i32,
            y: centre.1 + (radius * angle.sin()) as i32,
        });
        std::thread::sleep(Duration::from_millis(120));
    }
}

/// The three behaviours that only exist because this watches live work.
#[cfg(target_os = "macos")]
fn behaviours() {
    use lingxia_computer_use as cu;
    use std::time::Duration;

    let pause = |seconds: u64| std::thread::sleep(Duration::from_secs(seconds));
    let bounds = cu::displays().expect("displays")[0].bounds;

    println!("\n1. opens by itself on the first thing that changes the machine");
    println!("   (nothing has called `show`)");
    cu::pip::note_activity(cu::Acted::At {
        x: bounds.x + bounds.w / 2,
        y: bounds.y + bounds.h / 2,
    });
    pause(2);
    println!("   {:?}", cu::pip::status());

    println!("\n2. moves aside when the work is underneath it");
    let where_is_it = || {
        let query = cu::WindowQuery::parse(&format!("pid:{}", std::process::id()));
        let mine = cu::windows(&query).expect("windows");
        let panel = mine.first().expect("the viewer has a window");
        (panel.bounds.x, panel.bounds.y)
    };
    let before = where_is_it();
    println!("   panel at {before:?}; marking a point inside it");
    for _ in 0..12 {
        // Inside the panel, which starts in the bottom-right corner.
        cu::pip::note_activity(cu::Acted::At {
            x: before.0 + 60,
            y: before.1 + 60,
        });
        std::thread::sleep(Duration::from_millis(150));
    }
    pause(1);
    let after = where_is_it();
    println!("   panel now at {after:?}");
    assert_ne!(
        before, after,
        "a viewer sitting on the work must take itself out of the way"
    );

    println!("\n3. goes away when the work stops (idle rest, ~12s)");
    println!("   watch the panel disappear without anyone closing it");
    for second in 1..=16 {
        std::thread::sleep(Duration::from_secs(1));
        let status = cu::pip::status();
        if status.watching.is_none() {
            println!("   rested after {second}s: {status:?}");
            assert!(
                !status.dismissed,
                "idle rest must not read as a person closing it, or it would never come back"
            );
            break;
        }
    }

    println!("\n4. comes straight back on the next thing that happens");
    cu::pip::note_activity(cu::Acted::At {
        x: bounds.x + bounds.w / 4,
        y: bounds.y + bounds.h / 4,
    });
    pause(2);
    let status = cu::pip::status();
    println!("   {status:?}");
    assert!(status.visible, "the next action must bring it back");
    println!("\nall four behaved as described");
}
