//! The picture-in-picture viewer: a small window that mirrors what is being
//! driven, so a person can watch it happen.
//!
//! It is a viewer, not a control. Nothing in here moves a pointer or presses a
//! key, and no command reads pixels back out of it — a client that wants the
//! screen asks for a screenshot. What it buys is that someone whose machine is
//! being automated can see it as it happens rather than reconstruct it from a
//! log afterwards, which is also why it opens itself the first time a command
//! actuates something: a window that appears only when asked for is absent
//! exactly when it would have mattered.
//!
//! Three behaviours follow from being a viewer of live work rather than a
//! window someone opened:
//!
//! - **It follows the work.** A run that moves to another screen, or to a
//!   window a command names, takes the viewer with it. One viewer, wherever the
//!   action is — not one per app, and never a still picture of where the work
//!   started. A watch someone named themselves is pinned and stays put.
//! - **It leaves when the work stops.** There is no signal that says a run has
//!   ended, so it goes on idleness instead, and comes straight back on the next
//!   thing that happens. That is not the same as a person closing it: that is
//!   final for the run, because a viewer that keeps coming back is one people
//!   learn to dismiss rather than read.
//! - **It gets out of its own way.** It ignores the mouse — the agent clicks
//!   real coordinates on the real screen, and a viewer sitting over one of them
//!   would swallow the click and make the automation wrong in a way that looks
//!   like the app's fault. Being unclickable means it cannot be dragged either,
//!   so when the work happens underneath it, it hops to the far corner by
//!   itself. `corner` still says where to start.

use std::cell::RefCell;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSImage, NSImageScaling, NSImageView, NSPanel, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize};

use crate::error::{Error, Result};
use crate::model::{Acted, Pip, PipCorner as Corner, PipWatch, WindowTarget};

use super::capture;

/// Refresh rate. Fast enough that a click and what it opened read as one
/// motion, slow enough that a full-screen capture at 8Hz stays a rounding error
/// next to what the agent it is watching costs.
const FPS: u32 = 8;
/// The viewer's width in points. Big enough to recognise what is happening,
/// small enough to leave someone their screen.
const WIDTH: f64 = 360.0;
/// Gap from the screen edge, matched to the shadowless borderless panel.
const INSET: f64 = 16.0;
/// How long the marker stays on the last point that was acted on. Long enough
/// to catch at 8Hz, short enough that a stale dot never reads as a live one.
const MARKER_LINGER: Duration = Duration::from_millis(1200);
/// How long the viewer stays up after the last thing that changed the machine.
///
/// There is no signal that says "the run is over" — a run is a string of
/// commands with gaps between them — so the viewer leaves on idleness instead.
/// The threshold sits well above any gap inside one run, which is what keeps it
/// from flickering shut between two clicks, and well below how long someone
/// would tolerate a window they no longer need.
const IDLE_REST: Duration = Duration::from_secs(12);

struct State {
    watch: Option<PipWatch>,
    /// The watch was named by a person. Nothing re-points it, and idleness does
    /// not put it away: they asked for this one, and moving or closing it would
    /// be answering a question they did not ask.
    pinned: bool,
    dismissed: bool,
    corner: Corner,
    /// Bumped by every show and hide. A refresh thread stops as soon as its own
    /// generation is stale, which is what keeps two of them from ever running.
    generation: u64,
    /// The panel's own window number, so the capture can leave it out. Zero
    /// until the panel exists.
    window_number: u32,
    marker: Option<(i32, i32, Instant)>,
    /// When something last changed the machine, for the idle timeout.
    last_activity: Option<Instant>,
    /// The panel's own rect in global desktop points, so it can tell when it is
    /// sitting on top of what is being driven.
    panel_rect: Option<(f64, f64, f64, f64)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    watch: None,
    pinned: false,
    dismissed: false,
    corner: Corner::BottomRight,
    generation: 0,
    window_number: 0,
    marker: None,
    last_activity: None,
    panel_rect: None,
});

impl State {
    /// Whether the panel is sitting on a global desktop point.
    fn covers(&self, (x, y): (i32, i32)) -> bool {
        self.panel_rect.is_some_and(|(px, py, pw, ph)| {
            let (x, y) = (x as f64, y as f64);
            x >= px && x < px + pw && y >= py && y < py + ph
        })
    }
}

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

/// The AppKit side. Main-thread only by construction: nothing else can reach a
/// thread local of the main thread.
struct Viewer {
    panel: Retained<NSPanel>,
    image: Retained<NSImageView>,
}

thread_local! {
    static VIEWER: RefCell<Option<Viewer>> = const { RefCell::new(None) };
}

/// Run on the main queue, inline when already there.
fn on_main<T: Send>(work: impl FnOnce(MainThreadMarker) -> T + Send) -> Option<T> {
    if unsafe { libc::pthread_main_np() } != 0 {
        return MainThreadMarker::new().map(work);
    }
    let mut work = Some(work);
    let mut out = None;
    dispatch2::DispatchQueue::main().exec_sync(|| {
        if let (Some(work), Some(mtm)) = (work.take(), MainThreadMarker::new()) {
            out = Some(work(mtm));
        }
    });
    out
}

pub fn status() -> Pip {
    let state = state();
    Pip {
        visible: state.generation > 0 && state.watch.is_some() && state.window_number != 0,
        watching: state.watch.as_ref().map(describe),
        dismissed: state.dismissed,
        fps: FPS,
        supported: true,
    }
}

fn describe(watch: &PipWatch) -> String {
    match watch {
        PipWatch::Display(n) => format!("display {n}"),
        PipWatch::Window(id) => format!("window {id}"),
    }
}

/// Open the viewer at a watch a person named.
pub fn show(watch: PipWatch, corner: Option<Corner>) -> Result<Pip> {
    open(watch, corner, true)
}

/// `pinned` marks a watch someone chose deliberately, as opposed to one the
/// last command implied.
fn open(watch: PipWatch, corner: Option<Corner>, pinned: bool) -> Result<Pip> {
    // Resolve before showing anything: a viewer that opens onto an error is
    // worse than a command that says what was wrong.
    let _ = source_rect(&watch)?;
    let generation = {
        let mut state = state();
        state.watch = Some(watch);
        state.pinned = pinned;
        // Asking for it by name is how someone takes back a dismissal.
        state.dismissed = false;
        state.last_activity = Some(Instant::now());
        if let Some(corner) = corner {
            state.corner = corner;
        }
        state.generation += 1;
        state.generation
    };
    present()?;
    std::thread::Builder::new()
        .name("lingxia-pip".into())
        .spawn(move || refresh_loop(generation))
        .map_err(|error| Error::Failed(format!("could not start the viewer: {error}")))?;
    Ok(status())
}

/// Put the viewer away. This is also what idleness does — it leaves
/// `dismissed` alone, so the next thing that happens brings it back.
pub fn hide() -> Result<Pip> {
    {
        let mut state = state();
        state.generation += 1;
        state.watch = None;
        state.pinned = false;
        state.window_number = 0;
        state.marker = None;
        state.panel_rect = None;
    }
    on_main(|_| {
        VIEWER.with_borrow(|viewer| {
            if let Some(viewer) = viewer {
                viewer.panel.orderOut(None);
            }
        });
    });
    Ok(status())
}

/// Record that something was just acted on, and open the viewer if this is the
/// first time this run.
///
/// Called for the commands that change the machine and not for the ones that
/// only look at it: watching a screenshot being taken tells nobody anything,
/// and opening a window because an agent asked what the screen looks like would
/// make the viewer noise.
pub fn note_activity(acted: Acted) {
    if state().dismissed {
        return;
    }
    // Window-relative points become global here, where the window's bounds are
    // one lookup away, so everything downstream deals in one space.
    let (point, window) = match &acted {
        Acted::At { x, y } => (Some((*x, *y)), None),
        Acted::InWindow { id, x, y } => {
            let origin = window_origin(id);
            (origin.map(|(ox, oy)| (ox + x, oy + y)), Some(id.clone()))
        }
        Acted::Window(id) => (None, Some(id.clone())),
        Acted::Somewhere => (None, None),
    };

    // What this action says the viewer should be looking at: the window the
    // command named, else the display it happened on.
    let wanted = match window {
        Some(id) => Some(PipWatch::Window(id)),
        None => point.map(|(x, y)| PipWatch::Display(display_holding(x, y))),
    };

    let next = {
        let mut state = state();
        if let Some((x, y)) = point {
            state.marker = Some((x, y, Instant::now()));
        }
        state.last_activity = Some(Instant::now());
        match (&state.watch, &wanted) {
            // Not up: this action decides what it opens onto.
            (None, _) => Next::Open(wanted.clone().unwrap_or(PipWatch::Display(1))),
            // Up and following: work that moves to another screen — or to the
            // window a command named — takes the viewer with it. Without this
            // it keeps showing where the work started while reading as live.
            (Some(current), Some(wanted)) if !state.pinned && !watching_same(current, wanted) => {
                state.watch = Some(wanted.clone());
                Next::Repoint
            }
            // The common case, and it must stay free: this runs after every
            // command that changes anything, and a main-thread round trip here
            // would be a tax on all of them.
            _ => Next::Nothing,
        }
    };

    let moved = match next {
        Next::Nothing => return,
        Next::Repoint => present(),
        Next::Open(watch) => open(watch, None, false).map(|_| ()),
    };
    if let Err(error) = moved {
        log::debug!("picture-in-picture did not follow: {error}");
        // Not an error the caller hears about: the command it was watching
        // succeeded, and a viewer that cannot open must not fail it.
        state().dismissed = true;
    }
}

enum Next {
    Nothing,
    Repoint,
    Open(PipWatch),
}

fn watching_same(a: &PipWatch, b: &PipWatch) -> bool {
    match (a, b) {
        (PipWatch::Display(a), PipWatch::Display(b)) => a == b,
        (PipWatch::Window(a), PipWatch::Window(b)) => a == b,
        _ => false,
    }
}

/// A window's top-left in global points, if it still exists.
fn window_origin(id: &str) -> Option<(i32, i32)> {
    let window = super::window_ops::status(&WindowTarget::Id(id.to_string())).ok()?;
    Some((window.bounds.x, window.bounds.y))
}

/// The 1-based display index holding a point, defaulting to the first.
fn display_holding(x: i32, y: i32) -> usize {
    let Ok(displays) = super::displays() else {
        return 1;
    };
    displays
        .iter()
        .position(|display| {
            let bounds = display.bounds;
            x >= bounds.x && x < bounds.x + bounds.w && y >= bounds.y && y < bounds.y + bounds.h
        })
        .map_or(1, |index| index + 1)
}

/// The global-point rect the viewer is mirroring.
fn source_rect(watch: &PipWatch) -> Result<CGRect> {
    let rect = match watch {
        PipWatch::Display(n) => {
            let displays = super::displays()?;
            let display = displays
                .get(n.wrapping_sub(1))
                .ok_or_else(|| Error::NotFound(format!("no display {n}")))?;
            display.bounds
        }
        PipWatch::Window(id) => super::window_ops::status(&WindowTarget::Id(id.clone()))?.bounds,
    };
    Ok(CGRect::new(
        CGPoint::new(rect.x as f64, rect.y as f64),
        CGSize::new(rect.w as f64, rect.h as f64),
    ))
}

/// Create the panel if it does not exist, size it to what it is watching, and
/// bring it to the front.
fn present() -> Result<()> {
    let rect = {
        let watch = state().watch.clone();
        let watch = watch.ok_or_else(|| Error::Failed("nothing to watch".into()))?;
        source_rect(&watch)?
    };
    let corner = state().corner;
    let placed = on_main(move |mtm| {
        VIEWER.with_borrow_mut(|slot| {
            let viewer = slot.get_or_insert_with(|| build(mtm));
            let height = if rect.size.width > 0.0 {
                WIDTH * rect.size.height / rect.size.width
            } else {
                WIDTH * 0.625
            };
            let frame = place(mtm, corner, WIDTH, height);
            viewer.panel.setFrame_display(frame, true);
            viewer.image.setFrame(NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(frame.size.width, frame.size.height),
            ));
            viewer.panel.orderFrontRegardless();
            (
                viewer.panel.windowNumber().max(0) as u32,
                to_desktop(mtm, frame),
            )
        })
    })
    .ok_or_else(|| Error::Unavailable("no main thread to show the viewer on".into()))?;
    let mut state = state();
    state.window_number = placed.0;
    state.panel_rect = Some(placed.1);
    Ok(())
}

/// An AppKit rect (bottom-left origin) as global desktop points (top-left
/// origin), the space every coordinate outside AppKit is in.
fn to_desktop(mtm: MainThreadMarker, frame: NSRect) -> (f64, f64, f64, f64) {
    let flip = objc2_app_kit::NSScreen::screens(mtm)
        .iter()
        .next()
        .map_or(900.0, |primary| primary.frame().size.height);
    (
        frame.origin.x,
        flip - (frame.origin.y + frame.size.height),
        frame.size.width,
        frame.size.height,
    )
}

/// Send the panel to the corner furthest from a point it is covering.
fn move_away_from(x: i32, y: i32) {
    let screen = on_main(|mtm| {
        objc2_app_kit::NSScreen::mainScreen(mtm).map_or((1440.0, 900.0), |screen| {
            (screen.frame().size.width, screen.frame().size.height)
        })
    });
    let Some(screen) = screen else { return };
    let corner = corner_away_from(x as f64, y as f64, screen);
    if state().corner == corner {
        // Already as far away as a corner gets; a marker inside the panel here
        // means the panel is bigger than the quadrant, and hopping forever
        // would be worse than staying put.
        return;
    }
    state().corner = corner;
    if let Err(error) = present() {
        log::debug!("picture-in-picture could not move aside: {error}");
    }
}

/// The corner to move to when the viewer is sitting on the work: the one
/// diagonally away from the point, so one hop is always enough.
fn corner_away_from(x: f64, y: f64, screen: (f64, f64)) -> Corner {
    let left = x < screen.0 / 2.0;
    let top = y < screen.1 / 2.0;
    match (left, top) {
        (true, true) => Corner::BottomRight,
        (false, true) => Corner::BottomLeft,
        (true, false) => Corner::TopRight,
        (false, false) => Corner::TopLeft,
    }
}

fn build(mtm: MainThreadMarker) -> Viewer {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, WIDTH * 0.625));
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        frame,
        // Borderless, because the panel ignores the mouse: a title bar whose
        // close button cannot be pressed is a lie about the window.
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setLevel(objc2_app_kit::NSFloatingWindowLevel);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel.setHidesOnDeactivate(false);
    // A closed panel that frees itself leaves this holding a dangling pointer
    // the next time it is asked to open.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setIgnoresMouseEvents(true);
    panel.setOpaque(true);
    panel.setBackgroundColor(Some(&NSColor::blackColor()));

    let image = NSImageView::initWithFrame(NSImageView::alloc(mtm), frame);
    image.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    image.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    panel.setContentView(Some(image.as_ref() as &NSView));

    Viewer { panel, image }
}

/// Where the viewer sits, in the main screen's visible frame.
fn place(mtm: MainThreadMarker, corner: Corner, width: f64, height: f64) -> NSRect {
    let visible = objc2_app_kit::NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame())
        .unwrap_or(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(1440.0, 900.0),
        ));
    // AppKit's origin is bottom-left, which is why "top" is the far edge here.
    let (x, y) = match corner {
        Corner::TopLeft => (
            visible.origin.x + INSET,
            visible.origin.y + visible.size.height - height - INSET,
        ),
        Corner::TopRight => (
            visible.origin.x + visible.size.width - width - INSET,
            visible.origin.y + visible.size.height - height - INSET,
        ),
        Corner::BottomLeft => (visible.origin.x + INSET, visible.origin.y + INSET),
        Corner::BottomRight => (
            visible.origin.x + visible.size.width - width - INSET,
            visible.origin.y + INSET,
        ),
    };
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/// Capture, draw, hand to the panel, repeat — until this generation is stale or
/// the person closes the window.
fn refresh_loop(generation: u64) {
    let interval = Duration::from_millis(1000 / FPS as u64);
    loop {
        let (watch, below, marker, idle, sitting_on) = {
            let state = state();
            if state.generation != generation {
                return;
            }
            let marker = state
                .marker
                .filter(|(_, _, at)| at.elapsed() < MARKER_LINGER)
                .map(|(x, y, _)| (x, y));
            let idle = !state.pinned
                && state
                    .last_activity
                    .is_some_and(|at| at.elapsed() > IDLE_REST);
            let sitting_on = marker.filter(|point| state.covers(*point));
            (
                state.watch.clone(),
                state.window_number,
                marker,
                idle,
                sitting_on,
            )
        };
        let Some(watch) = watch else { return };

        if idle {
            let _ = hide();
            return;
        }

        // The viewer is over the thing being driven. It cannot be dragged out
        // of the way — it ignores the mouse so it can never swallow a click —
        // so it takes itself out of the way instead.
        if let Some((x, y)) = sitting_on {
            move_away_from(x, y);
        }

        let started = Instant::now();
        match frame(&watch, below, marker) {
            Ok(png) => {
                if !hand_to_panel(generation, png) {
                    return;
                }
            }
            // A window that closed, or a display that was unplugged, ends the
            // viewer rather than leaving a frozen last frame up.
            Err(error) => {
                log::debug!("picture-in-picture stopping: {error}");
                let _ = hide();
                return;
            }
        }
        // What is left of the interval, not the whole of it: a frame costs
        // real time, and sleeping the full period on top of it would make the
        // rate this reports a rate it never reaches.
        std::thread::sleep(interval.saturating_sub(started.elapsed()));
    }
}

/// Set the image, and notice a panel the person has closed. Returns false when
/// the loop should stop.
fn hand_to_panel(generation: u64, png: Vec<u8>) -> bool {
    on_main(move |_| {
        VIEWER.with_borrow(|viewer| {
            let Some(viewer) = viewer else { return false };
            if !viewer.panel.isVisible() {
                let mut state = state();
                // Only if nothing else has moved on: `hide` orders the panel
                // out too, and that is not a dismissal.
                if state.generation == generation {
                    state.generation += 1;
                    state.dismissed = true;
                    state.watch = None;
                    state.window_number = 0;
                }
                return false;
            }
            let data = NSData::with_bytes(&png);
            if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                viewer.image.setImage(Some(&image));
            }
            true
        })
    })
    .unwrap_or(false)
}

/// One frame: capture at the size it will be shown, mark the last point acted
/// on, encode.
fn frame(watch: &PipWatch, below: u32, marker: Option<(i32, i32)>) -> Result<Vec<u8>> {
    let rect = source_rect(watch)?;
    // Twice the panel's point width: enough for a Retina panel, and the scaling
    // happens inside the capture rather than over a full-screen buffer here.
    let limit = (WIDTH * 2.0) as u32;
    let (width, height, mut rgba) = match watch {
        PipWatch::Display(_) => capture::rgba_below_window(rect, below, limit)?,
        PipWatch::Window(id) => capture::rgba_of_window(super::parse_window_id(id)?, limit)?,
    };
    if width == 0 || height == 0 {
        return Err(Error::Failed("captured nothing".into()));
    }

    // The buffer covers `rect` whatever size it came back, so its own width is
    // what maps a global point into it.
    if let Some((x, y)) = marker {
        let px = (x as f64 - rect.origin.x) * width as f64 / rect.size.width.max(1.0);
        let py = (y as f64 - rect.origin.y) * height as f64 / rect.size.height.max(1.0);
        if px >= 0.0 && py >= 0.0 && px < width as f64 && py < height as f64 {
            draw_marker(&mut rgba, width, height, px as i32, py as i32);
        }
    }

    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| Error::Failed("frame buffer size mismatch".into()))?;
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| Error::Failed(format!("frame encode failed: {error}")))?;
    Ok(png)
}

/// A two-tone ring on the point just acted on. Two tones because one colour is
/// invisible against some wallpaper, and a ring rather than a dot because it
/// must not hide the thing it is pointing at.
fn draw_marker(rgba: &mut [u8], width: u32, height: u32, cx: i32, cy: i32) {
    let outer = (width.min(height) as f64 * 0.035).clamp(10.0, 24.0);
    let inner = outer * 0.62;
    let radius = outer.ceil() as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                continue;
            }
            let distance = ((dx * dx + dy * dy) as f64).sqrt();
            if distance > outer || distance < inner {
                continue;
            }
            // The outer half dark, the inner half bright, so the ring keeps its
            // edge whatever it lands on.
            let bright = distance < (inner + outer) / 2.0;
            let i = ((y as u32 * width + x as u32) * 4) as usize;
            let (r, g, b) = if bright { (255, 214, 10) } else { (40, 30, 0) };
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
        }
    }
}
