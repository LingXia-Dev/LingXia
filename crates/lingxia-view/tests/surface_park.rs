//! A trimmed page parks its surface at 1x1; it must draw again at whatever size it returns to.
#![cfg(all(feature = "gpu", target_os = "macos"))]

use lingxia_view::ViewSession;
use lingxia_view::renderer::gpu::{MetalLayer, PARK_EVERY_BACKEND, SurfaceSource};
use lingxia_view::renderer::surfaces;
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

/// The frame's size and the straight RGBA of the pixel at `(x, y)`.
fn pixel(id: u64, x: u32, y: u32) -> (u32, u32, [u8; 4]) {
    let (width, height, pixels) = surfaces::readback(id).unwrap().expect("a prepared frame");
    let at = ((y * width + x) * 4) as usize;
    (width, height, pixels[at..at + 4].try_into().unwrap())
}

#[test]
fn a_trimmed_page_draws_again_at_the_size_it_returns_to() {
    if unsafe { MTLCreateSystemDefaultDevice() }.is_null() {
        return;
    }
    // Metal has nothing to free; parked anyway, the same park and unpark run here.
    PARK_EVERY_BACKEND.store(true, std::sync::atomic::Ordering::Relaxed);
    let module = format!(
        "{}\nfunction render(){{return h('view',{{style:{{flexGrow:1,backgroundColor:'#3b82f6'}}}})}}\n__lingxiaPublish(render);",
        include_str!("../src/session/tests/runtime.js")
    );
    let mut page = ViewSession::new(&module).unwrap();
    let layer = send0(unsafe { objc_getClass(c"CAMetalLayer".as_ptr()) }, c"new");
    let source = SurfaceSource::Layer(Arc::new(Layer(NonNull::new(layer).unwrap())));
    let portrait = page.layout(390, 844, 1.0).unwrap();
    let id = surfaces::attach(source, 390, 844, 0).unwrap();
    surfaces::prepare(id, &portrait, 1).unwrap();
    surfaces::present(id, 1).unwrap();
    let blue = [0x3b, 0x82, 0xf6, 0xff];

    surfaces::trim(id);
    surfaces::prepare(id, &portrait, 2).unwrap();
    surfaces::present(id, 2).unwrap();
    assert_eq!(
        pixel(id, 195, 800),
        (390, 844, blue),
        "back at its own size"
    );

    // Rotated while hidden: the parked size is stale and the surface takes the new one.
    surfaces::trim(id);
    let landscape = page.layout(844, 390, 1.0).unwrap();
    surfaces::prepare(id, &landscape, 3).unwrap();
    surfaces::present(id, 3).unwrap();
    assert_eq!(pixel(id, 800, 195), (844, 390, blue), "back at a new size");

    surfaces::suspend(id);
    surfaces::detach(id);
    send0(layer, c"release");
}
