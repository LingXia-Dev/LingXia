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
//!   started.
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
    NSBackingStoreType, NSBitmapFormat, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
    NSImage, NSImageScaling, NSImageView, NSLineBreakMode, NSPanel, NSTextField, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::error::{Error, Result};
use crate::model::{Acted, WindowTarget};
use crate::supervision_state::{ActivityState, ActivityTarget, SessionKind, Transition};

/// Which corner it sits in. It is placed rather than dragged: it ignores the
/// mouse so it can never swallow a click meant for what is underneath, and a
/// window nobody can grab needs another way to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

use super::capture;

/// Refresh rate. Fast enough that a click and what it opened read as one
/// motion, slow enough that a full-screen capture at 8Hz stays a rounding error
/// next to what the agent it is watching costs.
const FPS: u32 = 8;
/// The viewer's width in points. Big enough to recognise what is happening,
/// small enough to leave someone their screen.
const WIDTH: f64 = 360.0;
/// Persistent identity chrome above a live preview.
const HEADER_HEIGHT: f64 = 34.0;
/// The low-obstruction state used when the controlled window is already in
/// front of the person.
const COMPACT_HEIGHT: f64 = 48.0;
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
    activity: ActivityState,
    session: Option<SessionKind>,
    corner: Corner,
    mode: Option<ViewerMode>,
    /// The panel's own window number, so the capture can leave it out. Zero
    /// until the panel exists.
    window_number: u32,
    /// The panel's own rect in global desktop points, so it can tell when it is
    /// sitting on top of what is being driven.
    panel_rect: Option<(f64, f64, f64, f64)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    activity: ActivityState::new(),
    session: None,
    corner: Corner::BottomRight,
    mode: None,
    window_number: 0,
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

    /// Hiding does not destroy the retained NSPanel. Keep its WindowServer
    /// number so enumeration already knows what to exclude when it is ordered
    /// front again; only its visible placement becomes stale.
    fn note_hidden(&mut self) {
        self.mode = None;
        self.panel_rect = None;
    }

    fn set_corner_if_current(&mut self, generation: u64, epoch: u64, corner: Corner) -> bool {
        if !self.activity.current(generation, epoch) || self.corner == corner {
            return false;
        }
        self.corner = corner;
        true
    }
}

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

pub(super) fn window_number() -> u32 {
    state().window_number
}

/// The AppKit side. Main-thread only by construction: nothing else can reach a
/// thread local of the main thread.
struct Viewer {
    panel: Retained<NSPanel>,
    root: Retained<NSView>,
    image: Retained<NSImageView>,
    accent: Retained<NSTextField>,
    label: Retained<NSTextField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerMode {
    Compact,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PanelLayout {
    width: f64,
    image_height: f64,
    height: f64,
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

fn open(generation: u64) -> Result<()> {
    // Repoint changes the UI epoch but deliberately keeps the generation. The
    // Open reservation owns converging to the latest target and starting the
    // sole refresh worker even when a Repoint wake-up could not be spawned.
    loop {
        let epoch = {
            let state = state();
            if !state.activity.active_generation(generation) {
                return Ok(());
            }
            state.activity.epoch
        };
        if let Err(error) = present_with_fallback(generation, epoch) {
            let hide = state().activity.rest(generation, epoch);
            if let Some(epoch) = hide {
                put_away(epoch);
                return Err(error);
            }
            if state().activity.active_generation(generation) {
                continue;
            }
            return Ok(());
        }
        let state = state();
        if !state.activity.active_generation(generation) {
            return Ok(());
        }
        if state.activity.epoch == epoch {
            break;
        }
    }
    let started = std::thread::Builder::new()
        .name("lingxia-pip".into())
        .spawn(move || refresh_loop(generation))
        .map_err(|error| Error::Failed(format!("could not start the viewer: {error}")));
    if started.is_err() {
        let hide = {
            let mut state = state();
            let epoch = state.activity.epoch;
            state.activity.rest(generation, epoch)
        };
        if let Some(epoch) = hide {
            put_away(epoch);
        }
    }
    started?;
    Ok(())
}

/// Put away only the UI revision that `epoch` retired.
///
/// A refresh thread can decide to stop long after a newer command has opened a
/// newer viewer; without this it would tear that one down instead of its own.
fn put_away(epoch: u64) {
    on_main(move |_| {
        let mut state = state();
        if state.activity.epoch != epoch || state.activity.target.is_some() {
            return;
        }
        state.note_hidden();
        VIEWER.with_borrow(|viewer| {
            if let Some(viewer) = viewer {
                viewer.panel.orderOut(None);
            }
        });
    });
}

/// Trusted-host hold for persistent disclosure. Not a remote dismiss path.
pub fn begin_session(kind: SessionKind) {
    let should_open = {
        let mut state = state();
        let first = state.session.is_none();
        state.session = Some(kind);
        first && state.activity.target.is_none()
    };
    if should_open {
        note_activity(Acted::Somewhere);
    }
}

pub fn end_session() {
    let hide = {
        let mut state = state();
        state.session = None;
        state.activity.target.is_none()
    };
    if hide {
        let epoch = state().activity.epoch;
        put_away(epoch);
    }
}

/// Put the activity preview away at a person's request. Persistent
/// disclosure stays until the host ends the session.
pub fn dismiss() {
    let epoch = state().activity.dismiss();
    if state().session.is_some() {
        return;
    }
    put_away(epoch);
}

/// Record that something was just acted on, and open the viewer if this is the
/// first time this run.
///
/// Called for the commands that change the machine and not for the ones that
/// only look at it: watching a screenshot being taken tells nobody anything,
/// and opening a window because an agent asked what the screen looks like would
/// make the viewer noise.
pub fn note_activity(acted: Acted) {
    // Capture order before target discovery, which can block on OS APIs. Two
    // callers may finish discovery in the opposite order they started; the
    // reducer uses this timestamp to keep the later action authoritative.
    let observed_at = Instant::now();
    let (point, wanted) = match acted {
        Acted::At { x, y } => (
            Some((x, y)),
            Some(ActivityTarget::Display(display_holding(x, y))),
        ),
        Acted::Display { x, y } => (None, Some(ActivityTarget::Display(display_holding(x, y)))),
        Acted::AtWindow { x, y, id } => (
            Some((x, y)),
            Some(ActivityTarget::Window {
                id,
                fallback_display: display_holding(x, y),
            }),
        ),
        Acted::WindowWithFallback { id, x, y } => (
            None,
            Some(ActivityTarget::Window {
                id,
                fallback_display: display_holding(x, y),
            }),
        ),
        Acted::Somewhere => (None, None),
    };

    let next = {
        let mut state = state();
        state.activity.note(wanted, point, observed_at, IDLE_REST)
    };

    if matches!(next, Transition::Nothing | Transition::Ignored) {
        return;
    }
    // Off this thread, always. Opening or moving the panel waits on the
    // application's main queue, and this runs on the thread answering a command
    // that has *already done its work* — blocking here would let a stalled UI
    // hang a caller for a picture. The viewer is never worth that.
    let open_token = match next {
        Transition::Open { generation, epoch } => Some((generation, epoch)),
        _ => None,
    };
    let spawned = std::thread::Builder::new()
        .name("lingxia-pip-open".into())
        .spawn(move || {
            let moved = match next {
                Transition::Ignored | Transition::Nothing => return,
                Transition::Repoint { epoch } => {
                    let generation = state().activity.generation;
                    present_with_fallback(generation, epoch)
                }
                Transition::Open { generation, .. } => open(generation),
            };
            if let Err(error) = moved {
                log::debug!("picture-in-picture did not follow: {error}");
                // Not an error anyone hears about: the command it was watching
                // succeeded, and a viewer that cannot open must not fail it.
            }
        });
    if let Err(error) = spawned {
        log::debug!("picture-in-picture worker could not start: {error}");
        if let Some((generation, _)) = open_token {
            let hide = {
                let mut state = state();
                let epoch = state.activity.epoch;
                state.activity.rest(generation, epoch)
            };
            if let Some(epoch) = hide {
                put_away(epoch);
            }
        }
    }
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
fn source_rect(watch: &ActivityTarget) -> Result<CGRect> {
    let rect = match watch {
        ActivityTarget::Display(n) => {
            let displays = super::displays()?;
            let display = displays
                .get(n.wrapping_sub(1))
                .ok_or_else(|| Error::NotFound(format!("no display {n}")))?;
            display.bounds
        }
        ActivityTarget::Window { id, .. } => {
            super::window_ops::status(&WindowTarget::Id(id.clone()))?.bounds
        }
    };
    Ok(CGRect::new(
        CGPoint::new(rect.x as f64, rect.y as f64),
        CGSize::new(rect.w as f64, rect.h as f64),
    ))
}

fn presentation(watch: &ActivityTarget) -> Result<(CGRect, ViewerMode, String)> {
    let (rect, mode, target) = match watch {
        ActivityTarget::Display(index) => (
            source_rect(watch)?,
            ViewerMode::Full,
            format!("Display {index}"),
        ),
        ActivityTarget::Window { id, .. } => {
            let window = super::window_ops::status(&WindowTarget::Id(id.clone()))?;
            let target = if window.process.is_empty() {
                window.title
            } else {
                window.process
            };
            (
                CGRect::new(
                    CGPoint::new(window.bounds.x as f64, window.bounds.y as f64),
                    CGSize::new(window.bounds.w as f64, window.bounds.h as f64),
                ),
                mode_for(window.focused && window.visible),
                if target.is_empty() {
                    "Window".into()
                } else {
                    target
                },
            )
        }
    };
    let kind = state().session.unwrap_or(SessionKind::Control);
    let label = kind.label(&controller_identity(), &target);
    Ok((rect, mode, label))
}

fn mode_for(target_foreground: bool) -> ViewerMode {
    if target_foreground {
        ViewerMode::Compact
    } else {
        ViewerMode::Full
    }
}

fn target_mode(watch: &ActivityTarget) -> Result<ViewerMode> {
    match watch {
        ActivityTarget::Display(_) => Ok(ViewerMode::Full),
        ActivityTarget::Window { id, .. } => {
            let window = super::window_ops::status(&WindowTarget::Id(id.clone()))?;
            Ok(mode_for(window.focused && window.visible))
        }
    }
}

fn controller_identity() -> String {
    lingxia_app_context::product_name()
        .map(str::to_owned)
        .or_else(super::responsible_app_name)
        .or_else(|| {
            std::env::current_exe().ok().and_then(|path| {
                path.file_stem()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "This app".into())
}

/// Show the requested window, or the display that held it if the window has
/// disappeared between the completed command and this asynchronous update.
/// This is an internal capture failure, never evidence that a person dismissed
/// the viewer.
fn present_with_fallback(generation: u64, epoch: u64) -> Result<()> {
    match present(generation, epoch) {
        Ok(()) => Ok(()),
        Err(primary) => {
            let fallback_epoch = state().activity.fallback_to_display(generation, epoch);
            let Some(fallback_epoch) = fallback_epoch else {
                return Err(primary);
            };
            present(generation, fallback_epoch).map_err(|fallback| {
                Error::Unavailable(format!(
                    "window viewer failed ({primary}); display fallback failed ({fallback})"
                ))
            })
        }
    }
}

/// Create the panel if it does not exist, size it to what it is watching, and
/// bring it to the front.
fn present(generation: u64, epoch: u64) -> Result<()> {
    let (watch, corner) = {
        let state = state();
        if !state.activity.current(generation, epoch) {
            return Ok(());
        }
        (
            state.activity.target.clone().expect("current target"),
            state.corner,
        )
    };
    let (rect, mode, label) = presentation(&watch)?;
    let shown = on_main(move |mtm| {
        let mut state = state();
        if !state.activity.current(generation, epoch) {
            return None;
        }
        VIEWER.with_borrow_mut(|slot| {
            let viewer = slot.get_or_insert_with(|| build(mtm));
            let layout = panel_layout_for(mtm, mode, rect);
            let frame = place(mtm, corner, layout, rect);
            viewer.panel.setFrame_display(frame, true);
            layout_viewer(viewer, layout, &label);
            viewer.panel.orderFrontRegardless();
            // Publish the number before releasing the reducer lock. Public
            // enumeration waits for this lock, so it can never observe a
            // visible viewer while its exclusion id is still zero.
            state.window_number = viewer.panel.windowNumber().max(0) as u32;
            state.mode = Some(mode);
            state.panel_rect = Some(to_desktop(mtm, frame));
            Some(())
        })
    })
    .ok_or_else(|| Error::Unavailable("no main thread to show the viewer on".into()))?;
    let Some(()) = shown else { return Ok(()) };
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

/// The visible frame of the screen the watched rect sits on, in AppKit
/// coordinates. Falls back to the main screen when nothing matches — an
/// unplugged display should move the viewer, not lose it.
fn visible_frame_for(mtm: MainThreadMarker, watched: CGRect) -> NSRect {
    let screens = objc2_app_kit::NSScreen::screens(mtm);
    // Global desktop coordinates hang off the primary screen's top-left, so
    // that is the height the flip is measured against.
    let flip = screens
        .iter()
        .next()
        .map_or(900.0, |primary| primary.frame().size.height);
    let centre_x = watched.origin.x + watched.size.width / 2.0;
    let centre_y = flip - (watched.origin.y + watched.size.height / 2.0);
    screens
        .iter()
        .find(|screen| {
            let frame = screen.frame();
            centre_x >= frame.origin.x
                && centre_x < frame.origin.x + frame.size.width
                && centre_y >= frame.origin.y
                && centre_y < frame.origin.y + frame.size.height
        })
        .or_else(|| objc2_app_kit::NSScreen::mainScreen(mtm))
        .map(|screen| screen.visibleFrame())
        .unwrap_or(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(1440.0, 900.0),
        ))
}

/// Send the panel to the corner furthest from a point it is covering.
fn move_away_from(x: i32, y: i32) {
    // Measured against the screen the panel is on, which after following the
    // work may not be the primary one.
    let (watch, generation, epoch) = {
        let state = state();
        (
            state.activity.target.clone(),
            state.activity.generation,
            state.activity.epoch,
        )
    };
    let Some(watched) = watch.and_then(|watch| source_rect(&watch).ok()) else {
        return;
    };
    let Some(extent) = on_main(move |mtm| {
        let visible = visible_frame_for(mtm, watched);
        (visible.size.width, visible.size.height)
    }) else {
        return;
    };
    // The point relative to that screen, so the quadrant is the screen's rather
    // than the whole desktop's.
    let corner = corner_away_from(
        x as f64 - watched.origin.x,
        y as f64 - watched.origin.y,
        extent,
    );
    // Target lookup and main-queue work above can block. A newer action may
    // have repointed the viewer while this worker was away; never move that new
    // viewer using the old target's marker or geometry.
    if !state().set_corner_if_current(generation, epoch, corner) {
        return;
    }
    if let Err(error) = present(generation, epoch) {
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
    let frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(WIDTH, WIDTH * 0.625 + HEADER_HEIGHT),
    );
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        frame,
        // Borderless, because the panel ignores the mouse: a title bar whose
        // close button cannot be pressed is a lie about the window.
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setTitle(&NSString::from_str(super::VIEWER_WINDOW_SENTINEL));
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

    let root = NSView::initWithFrame(NSView::alloc(mtm), frame);
    let image = NSImageView::initWithFrame(
        NSImageView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, WIDTH * 0.625)),
    );
    image.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    root.addSubview(image.as_ref() as &NSView);

    let accent = NSTextField::labelWithString(&NSString::from_str("●"), mtm);
    accent.setTextColor(Some(&NSColor::systemYellowColor()));
    accent.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
    root.addSubview(accent.as_ref() as &NSView);

    let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    label.setTextColor(Some(&NSColor::whiteColor()));
    label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    root.addSubview(label.as_ref() as &NSView);

    panel.setContentView(Some(&root));

    Viewer {
        panel,
        root,
        image,
        accent,
        label,
    }
}

fn layout_viewer(viewer: &Viewer, layout: PanelLayout, label: &str) {
    let size = NSSize::new(layout.width, layout.height);
    viewer
        .root
        .setFrame(NSRect::new(NSPoint::new(0.0, 0.0), size));
    viewer.image.setHidden(layout.image_height == 0.0);
    viewer.image.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(layout.width, layout.image_height),
    ));
    let bar_height = layout.height - layout.image_height;
    let text_height = 20.0_f64.min(bar_height);
    let text_y = layout.image_height + (bar_height - text_height) / 2.0;
    viewer.accent.setFrame(NSRect::new(
        NSPoint::new(16.0, text_y),
        NSSize::new(16.0, text_height),
    ));
    viewer.label.setFrame(NSRect::new(
        NSPoint::new(42.0, text_y),
        NSSize::new((layout.width - 56.0).max(1.0), text_height),
    ));
    viewer.label.setStringValue(&NSString::from_str(label));
}

fn panel_layout_for(mtm: MainThreadMarker, mode: ViewerMode, watched: CGRect) -> PanelLayout {
    let visible = visible_frame_for(mtm, watched);
    panel_layout(
        mode,
        watched.size.width,
        watched.size.height,
        (visible.size.width - INSET * 2.0).max(1.0),
        (visible.size.height - INSET * 2.0).max(1.0),
    )
}

fn panel_layout(
    mode: ViewerMode,
    source_width: f64,
    source_height: f64,
    available_width: f64,
    available_height: f64,
) -> PanelLayout {
    if mode == ViewerMode::Compact {
        return PanelLayout {
            width: WIDTH.min(available_width.max(1.0)),
            image_height: 0.0,
            height: COMPACT_HEIGHT.min(available_height.max(1.0)),
        };
    }

    let header = HEADER_HEIGHT.min(available_height.max(1.0));
    let available_image_height = (available_height - header).max(1.0);
    let aspect_height = if source_width > 0.0 {
        WIDTH * source_height.max(1.0) / source_width
    } else {
        WIDTH * 0.625
    };
    let (width, image_height) = fit_size(
        WIDTH,
        aspect_height,
        available_width,
        available_image_height,
    );
    PanelLayout {
        width,
        image_height,
        height: (image_height + header).min(available_height.max(1.0)),
    }
}

/// Where the viewer sits: a corner of the screen showing the work, not a corner
/// of the primary one. Watching a second monitor from the first is a viewer
/// nobody is looking at.
fn place(mtm: MainThreadMarker, corner: Corner, layout: PanelLayout, watched: CGRect) -> NSRect {
    let visible = visible_frame_for(mtm, watched);
    let (width, height) = (layout.width, layout.height);
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

fn fit_size(width: f64, height: f64, available_width: f64, available_height: f64) -> (f64, f64) {
    let width = width.max(1.0);
    let height = height.max(1.0);
    let scale = (available_width.max(1.0) / width)
        .min(available_height.max(1.0) / height)
        .min(1.0);
    ((width * scale).max(1.0), (height * scale).max(1.0))
}

/// Capture, draw, hand to the panel, repeat — until this generation is stale or
/// the person closes the window.
fn refresh_loop(generation: u64) {
    let interval = Duration::from_millis(1000 / FPS as u64);
    loop {
        let (watch, epoch, below, marker, idle, sitting_on, shown_mode) = {
            let state = state();
            if state.activity.generation != generation || state.activity.dismissed {
                return;
            }
            let marker = state
                .activity
                .marker
                .filter(|(_, _, at)| at.elapsed() < MARKER_LINGER)
                .map(|(x, y, _)| (x, y));
            let idle = state
                .activity
                .last_activity
                .is_some_and(|at| at.elapsed() > IDLE_REST);
            let sitting_on = marker.filter(|point| state.covers(*point));
            (
                state.activity.target.clone(),
                state.activity.epoch,
                state.window_number,
                marker,
                idle,
                sitting_on,
                state.mode,
            )
        };
        let Some(watch) = watch else { return };

        if idle {
            if state().session.is_some() {
                // Disclosure stays up for the whole session. Resting would
                // clear the target and end this loop, leaving the panel on
                // screen showing a frame from before the lull.
                std::thread::sleep(interval);
                continue;
            }
            if let Some(epoch) = state().activity.rest(generation, epoch) {
                put_away(epoch);
            }
            return;
        }

        // The viewer is over the thing being driven. It cannot be dragged out
        // of the way — it ignores the mouse so it can never swallow a click —
        // so it takes itself out of the way instead.
        if let Some((x, y)) = sitting_on {
            move_away_from(x, y);
        }

        let started = Instant::now();
        let desired_mode = target_mode(&watch).unwrap_or(ViewerMode::Full);
        if shown_mode != Some(desired_mode) {
            if let Err(error) = present_with_fallback(generation, epoch) {
                log::debug!("picture-in-picture mode change failed: {error}");
            }
            std::thread::sleep(interval.saturating_sub(started.elapsed()));
            continue;
        }
        if desired_mode == ViewerMode::Compact {
            std::thread::sleep(interval.saturating_sub(started.elapsed()));
            continue;
        }
        match frame(&watch, below, marker) {
            Ok((width, height, rgba)) => {
                if !hand_to_panel(generation, epoch, width, height, rgba) {
                    return;
                }
            }
            // A window may disappear after a completed close. Follow its
            // fallback display instead of leaving a frozen last frame up.
            Err(error) => {
                log::debug!("picture-in-picture capture failed: {error}");
                let fallback_epoch = state().activity.fallback_to_display(generation, epoch);
                if let Some(fallback_epoch) = fallback_epoch {
                    if let Err(error) = present(generation, fallback_epoch) {
                        log::debug!("picture-in-picture fallback could not resize: {error}");
                    }
                    continue;
                }
                let epoch = state().activity.rest(generation, epoch);
                if let Some(epoch) = epoch {
                    put_away(epoch);
                    return;
                }
                if state().activity.generation != generation {
                    return;
                }
                continue;
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
fn hand_to_panel(generation: u64, epoch: u64, width: u32, height: u32, rgba: Vec<u8>) -> bool {
    on_main(move |_| {
        VIEWER.with_borrow(|viewer| {
            let Some(viewer) = viewer else { return false };
            {
                let state = state();
                if state.activity.generation != generation || state.activity.dismissed {
                    return false;
                }
                if !state.activity.current(generation, epoch) {
                    return true;
                }
            }
            if !viewer.panel.isVisible() {
                let mut state = state();
                // Only if nothing else has moved on: `hide` orders the panel
                // out too, and that is not a dismissal.
                if state.activity.current(generation, epoch) {
                    state.activity.dismiss();
                    state.note_hidden();
                }
                return false;
            }
            if let Some(image) = image_from_rgba(width, height, &rgba) {
                viewer.image.setImage(Some(&image));
            }
            true
        })
    })
    .unwrap_or(false)
}

/// AppKit owns the copied bitmap bytes after this returns. Keeping the viewer
/// on raw RGBA avoids a PNG encode and decode on every 8 Hz frame.
fn image_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Retained<NSImage>> {
    let expected = width as usize * height as usize * 4;
    if width == 0 || height == 0 || rgba.len() != expected {
        return None;
    }
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            NSBitmapFormat::AlphaNonpremultiplied,
            (width * 4) as isize,
            32,
        )?;
        let destination = rep.bitmapData();
        if destination.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), destination, expected);
        let image =
            NSImage::initWithSize(NSImage::alloc(), NSSize::new(width as f64, height as f64));
        image.addRepresentation(&rep);
        Some(image)
    }
}

/// One frame: capture at the size it will be shown and mark the last point
/// acted on.
fn frame(
    watch: &ActivityTarget,
    below: u32,
    marker: Option<(i32, i32)>,
) -> Result<(u32, u32, Vec<u8>)> {
    let rect = source_rect(watch)?;
    // Twice the panel's point width: enough for a Retina panel, and the scaling
    // happens inside the capture rather than over a full-screen buffer here.
    let limit = (WIDTH * 2.0) as u32;
    let (width, height, mut rgba) = match watch {
        ActivityTarget::Display(_) => capture::rgba_below_window(rect, below, limit)?,
        ActivityTarget::Window { id, .. } => {
            capture::rgba_of_window(super::parse_window_id(id)?, limit)?
        }
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

    Ok((width, height, rgba))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_viewer_fits_inside_the_work_area() {
        assert_eq!(fit_size(360.0, 640.0, 800.0, 400.0), (225.0, 400.0));
        assert_eq!(fit_size(360.0, 202.5, 320.0, 180.0), (320.0, 180.0));
        let (width, height) = fit_size(360.0, 10_000.0, 300.0, 200.0);
        assert!(width <= 300.0 && height <= 200.0);
    }

    #[test]
    fn hiding_a_retained_panel_keeps_its_windowserver_identity() {
        let mut state = State {
            activity: ActivityState::new(),
            session: None,
            corner: Corner::BottomRight,
            mode: Some(ViewerMode::Full),
            window_number: 73,
            panel_rect: Some((10.0, 20.0, 360.0, 225.0)),
        };
        state.note_hidden();
        assert_eq!(state.window_number, 73);
        assert_eq!(state.mode, None);
        assert_eq!(state.panel_rect, None);
    }

    #[test]
    fn an_old_move_worker_cannot_reposition_a_new_target() {
        let mut state = State {
            activity: ActivityState::new(),
            session: None,
            corner: Corner::BottomRight,
            mode: None,
            window_number: 0,
            panel_rect: None,
        };
        let now = Instant::now();
        let Transition::Open { generation, epoch } = state.activity.note(
            Some(ActivityTarget::Display(1)),
            Some((20, 20)),
            now,
            IDLE_REST,
        ) else {
            panic!("first activity opens the viewer");
        };
        let _ = state.activity.note(
            Some(ActivityTarget::Display(2)),
            Some((2020, 20)),
            now + Duration::from_millis(1),
            IDLE_REST,
        );

        assert!(!state.set_corner_if_current(generation, epoch, Corner::TopLeft));
        assert_eq!(state.corner, Corner::BottomRight);
    }

    #[test]
    fn foreground_targets_use_the_low_obstruction_state() {
        assert_eq!(mode_for(true), ViewerMode::Compact);
        assert_eq!(mode_for(false), ViewerMode::Full);
    }

    #[test]
    fn full_layout_reserves_identity_chrome_without_distorting_preview() {
        let layout = panel_layout(ViewerMode::Full, 1920.0, 1080.0, 800.0, 600.0);
        assert_eq!(layout.width, 360.0);
        assert_eq!(layout.image_height, 202.5);
        assert_eq!(layout.height, 236.5);

        let portrait = panel_layout(ViewerMode::Full, 1080.0, 1920.0, 300.0, 200.0);
        assert!(portrait.width <= 300.0);
        assert!(portrait.height <= 200.0);
        assert_eq!(portrait.height - portrait.image_height, HEADER_HEIGHT);
    }

    #[test]
    fn compact_layout_has_no_capture_surface() {
        assert_eq!(
            panel_layout(ViewerMode::Compact, 1920.0, 1080.0, 800.0, 600.0),
            PanelLayout {
                width: 360.0,
                image_height: 0.0,
                height: 48.0,
            }
        );
    }
}
