//! A page that loads again numbers its frames from the start; the surface must show and
//! capture the new frames, not the last one of the page as it was.
#![cfg(all(feature = "gpu", target_os = "macos"))]

use lingxia_view::ViewSession;
use lingxia_view::renderer::gpu::{MetalLayer, SurfaceSource};
use lingxia_view::renderer::surfaces;
use std::ffi::{CStr, c_char, c_void};
use std::ptr::NonNull;
use std::sync::Arc;

// One `#[link]` per framework: clippy reads the repeated `kind` as a duplicate.
#[allow(clippy::duplicated_attributes)]
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
// SAFETY: the layer is held retained by this value.
unsafe impl MetalLayer for Layer {
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

fn pixel(id: u64) -> [u8; 4] {
    let (width, _, pixels) = surfaces::readback(id).unwrap().expect("a prepared frame");
    let at = ((40 * width + 40) * 4) as usize;
    pixels[at..at + 4].try_into().unwrap()
}

fn page(colour: &str) -> ViewSession {
    ViewSession::new(&format!(
        "{}\nfunction render(){{return h('view',{{style:{{flexGrow:1,backgroundColor:'{colour}'}}}})}}\n__lingxiaPublish(render);",
        include_str!("../src/session/tests/runtime.js")
    ))
    .unwrap()
}

#[test]
fn a_reloaded_page_presents_and_captures_its_own_frames() {
    if unsafe { MTLCreateSystemDefaultDevice() }.is_null() {
        return;
    }
    let layer = send0(unsafe { objc_getClass(c"CAMetalLayer".as_ptr()) }, c"new");
    let source = SurfaceSource::Layer(Arc::new(Layer(NonNull::new(layer).unwrap())));
    let id = surfaces::attach(source, 200, 200, 0).unwrap();
    let blue = page("#3b82f6").layout(200, 200, 1.0).unwrap();
    for revision in 1..=40 {
        surfaces::prepare(id, &blue, revision).unwrap();
        assert!(surfaces::present(id, revision).unwrap().is_some());
    }
    assert_eq!(pixel(id), [0x3b, 0x82, 0xf6, 0xff]);

    // The page loads again on the same surface: its frames count from 1.
    let red = page("#ef4444").layout(200, 200, 1.0).unwrap();
    surfaces::prepare(id, &red, 1).unwrap();
    assert!(surfaces::present(id, 1).unwrap().is_some());
    assert_eq!(
        pixel(id),
        [0xef, 0x44, 0x44, 0xff],
        "the capture is the new page"
    );
    // A frame prepared ahead does not take the slot of the one still waiting to present.
    surfaces::prepare(id, &red, 2).unwrap();
    surfaces::prepare(id, &red, 3).unwrap();
    assert!(
        surfaces::present(id, 2).unwrap().is_some(),
        "revision 2 is still held"
    );
    assert!(surfaces::present(id, 3).unwrap().is_some());
    assert_eq!(pixel(id), [0xef, 0x44, 0x44, 0xff]);

    surfaces::suspend(id);
    surfaces::detach(id);
    send0(layer, c"release");
}
