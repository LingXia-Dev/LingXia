//! A simulated device drawn at the screen's scale (the Runner) shows the same page after its
//! window was covered and a capture drew it at the device's scale.
#![cfg(all(feature = "gpu", target_os = "macos"))]

use lingxia_view::ViewSession;
use lingxia_view::renderer::gpu::{MetalLayer, SurfaceSource};
use lingxia_view::renderer::{render_thread, surfaces};
use std::ffi::{CStr, c_char, c_void};
use std::ptr::NonNull;
use std::sync::Arc;

#[link(name = "QuartzCore", kind = "framework")]
#[link(name = "Metal", kind = "framework")]
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_msgSend();
    fn sel_registerName(name: *const c_char) -> *const c_void;
    fn objc_getClass(name: *const c_char) -> *const c_void;
    fn MTLCreateSystemDefaultDevice() -> *mut c_void;
}

struct Layer(NonNull<c_void>);
unsafe impl Send for Layer {}
unsafe impl Sync for Layer {}
impl MetalLayer for Layer {
    fn layer(&self) -> NonNull<c_void> {
        self.0
    }
}

fn send0(object: *const c_void, name: &CStr) -> *mut c_void {
    unsafe {
        let send: unsafe extern "C" fn(*const c_void, *const c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(object, sel_registerName(name.as_ptr()))
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Clone, Copy)]
struct Size(f64, f64);

/// The pixels the layer shows: its drawable size.
fn drawable(layer: *const c_void) -> Size {
    unsafe {
        let send: unsafe extern "C" fn(*const c_void, *const c_void) -> Size =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(layer, sel_registerName(c"drawableSize".as_ptr()))
    }
}

fn show(id: u64, revision: u64, display: &lingxia_view::DisplayList) -> bool {
    let drawn = render_thread::draw(render_thread::SURFACES, id, revision, display.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    render_thread::present(id, revision, drawn, true, move |shown, _| {
        let _ = tx.send(shown);
    });
    rx.recv().unwrap().unwrap()
}

fn stat_page() -> lingxia_view::DisplayList {
    let module = format!(
        "{}\nfunction render(){{return h('view',{{style:{{flexGrow:1,backgroundColor:'#f1f5f9',padding:10}}}},h('view',{{style:{{height:40,backgroundColor:'#3b82f6'}}}}),h('text',{{style:{{fontSize:20,color:'text'}}}},'Stat page'))}}\n__lingxiaPublish(render);",
        include_str!("../src/session/tests/runtime.js")
    );
    let mut page = ViewSession::new(&module).unwrap();
    // The Runner: a 3x phone drawn at the 1x screen's pixels.
    let mut display = page.layout(600, 600, 3.0).unwrap();
    display.raster_at(1.0);
    display
}

/// Covered then shown again (one more frame of the same list), with or without a capture
/// at the device's pixels in between: the layer keeps the live raster and the page.
#[test]
fn a_page_drawn_off_the_device_scale_shows_again_after_hide_and_capture() {
    if unsafe { MTLCreateSystemDefaultDevice() }.is_null() {
        return;
    }
    let display = stat_page();
    for capture in [false, true] {
        let layer = send0(unsafe { objc_getClass(c"CAMetalLayer".as_ptr()) }, c"new");
        let source = SurfaceSource::Layer(Arc::new(Layer(NonNull::new(layer).unwrap())));
        let id = surfaces::attach(source, 200, 200, 0).unwrap();
        // First present right after attach, as a page opened in the background does.
        assert!(show(id, 2, &display));
        assert_eq!(drawable(layer), Size(200.0, 200.0));
        let shown = surfaces::readback(id).unwrap().expect("a shown frame");
        assert_eq!((shown.0, shown.1), (200, 200));
        let mut revision = 3;
        for _ in 0..3 {
            if capture {
                let (w, h, _) = surfaces::capture(id).unwrap().expect("a capture");
                assert_eq!((w, h), (600, 600), "the capture is at the device's pixels");
            }
            assert!(show(id, revision, &display));
            revision += 1;
            assert_eq!(drawable(layer), Size(200.0, 200.0), "capture={capture}");
            let again = surfaces::readback(id).unwrap().expect("a shown frame");
            assert_eq!((again.0, again.1), (200, 200));
            assert!(
                again.2 == shown.2,
                "the page shows again as it was (capture={capture})"
            );
        }
        surfaces::suspend(id);
        surfaces::detach(id);
        send0(layer, c"release");
    }
}

/// A covered window presents too, so its drawables may not come back: acquiring one times
/// out, and the present is refused, instead of blocking the render thread for good.
#[test]
fn a_surface_acquire_times_out_rather_than_blocking() {
    if unsafe { MTLCreateSystemDefaultDevice() }.is_null() {
        return;
    }
    let layer = send0(unsafe { objc_getClass(c"CAMetalLayer".as_ptr()) }, c"new");
    let source = SurfaceSource::Layer(Arc::new(Layer(NonNull::new(layer).unwrap())));
    let id = surfaces::attach(source, 200, 200, 0).unwrap();
    let times_out = unsafe {
        let send: unsafe extern "C" fn(*const c_void, *const c_void) -> i8 =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(
            layer,
            sel_registerName(c"allowsNextDrawableTimeout".as_ptr()),
        )
    };
    assert_eq!(times_out, 1);
    // A revision the presenter does not hold is refused, not an error.
    assert_eq!(surfaces::present(id, 99).unwrap(), None);
    surfaces::suspend(id);
    surfaces::detach(id);
    send0(layer, c"release");
}
