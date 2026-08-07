//! Puts the viewer on screen and photographs the result.
//!
//! The viewer is an AppKit panel, so nothing about it can be judged from a unit
//! test: whether it appears, whether it lands in the right corner, whether it
//! leaves itself out of its own capture, and whether the marker falls on the
//! point acted on are all questions only pixels answer. This runs a real
//! application, shows the viewer, marks a known point, then captures the whole
//! screen so the panel can be looked at.
//!
//!     cargo run -p lingxia-computer-use --example pip_viewer -- /tmp/proof.png

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("the viewer is macOS-only");
}

#[cfg(target_os = "macos")]
fn main() {
    use lingxia_computer_use as cu;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use std::time::Duration;

    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/pip-proof.png".to_string());

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    // Accessory: a panel with no dock icon and no menu bar, which is what this
    // looks like inside a real product too.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));

        match cu::pip::show(cu::PipWatch::Display(1), Some(cu::PipCorner::BottomRight)) {
            Ok(status) => println!("shown: {status:?}"),
            Err(error) => {
                eprintln!("could not show: {error}");
                std::process::exit(1);
            }
        }

        // A point in the middle of the first display: far from the corner the
        // viewer sits in, so a marker drawn there cannot be confused with the
        // panel's own edge.
        let displays = cu::displays().expect("displays");
        let bounds = displays[0].bounds;
        let (x, y) = (bounds.x + bounds.w / 2, bounds.y + bounds.h / 2);
        std::thread::sleep(Duration::from_millis(1200));
        // Kept fresh while the capture is set up, so a missing ring means the
        // ring is missing rather than that it had already faded.
        std::thread::spawn(move || {
            for _ in 0..40 {
                cu::pip::note_activity(cu::Acted::At { x, y });
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        println!("marked {x},{y}");

        // Capture the panel itself, not the screen: a full-screen capture takes
        // long enough that several unmarked frames replace the marked one while
        // it runs, which is how the marker came to look missing when it was not.
        std::thread::sleep(Duration::from_millis(1500));
        let query = cu::WindowQuery::parse(&format!("pid:{}", std::process::id()));
        let mine = cu::windows(&query).expect("windows");
        let panel = mine.first().expect("the viewer has a window");
        let capture =
            cu::screenshot(cu::CaptureTarget::Window(panel.id.clone())).expect("screenshot");
        std::fs::write(&output, &capture.png).expect("write");
        println!("wrote {output} ({}x{})", capture.width, capture.height);

        println!("status: {:?}", cu::pip::status());
        std::process::exit(0);
    });

    app.run();
}
