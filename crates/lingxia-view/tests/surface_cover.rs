//! A hidden page kept under the host's cover of its last frame parks its surface on every
//! backend, and draws again when it returns.
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
#[derive(Debug, PartialEq)]
struct Size {
    width: f64,
    height: f64,
}

fn drawable_size(layer: *const c_void) -> Size {
    unsafe {
        let send: unsafe extern "C" fn(*const c_void, *const c_void) -> Size =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(layer, sel_registerName(c"drawableSize".as_ptr()))
    }
}

/// The frame's size and the straight RGBA of the pixel at `(x, y)`.
fn pixel(id: u64, x: u32, y: u32) -> (u32, u32, [u8; 4]) {
    let (width, height, pixels) = surfaces::readback(id).unwrap().expect("a prepared frame");
    let at = ((y * width + x) * 4) as usize;
    (width, height, pixels[at..at + 4].try_into().unwrap())
}

#[test]
fn a_covered_page_parks_its_metal_surface_and_draws_again() {
    if unsafe { MTLCreateSystemDefaultDevice() }.is_null() {
        return;
    }
    let module = format!(
        "{}\nfunction render(){{return h('view',{{style:{{flexGrow:1,backgroundColor:'#3b82f6'}}}})}}\n__lingxiaPublish(render);",
        include_str!("../src/session/tests/runtime.js")
    );
    let mut page = ViewSession::new(&module).unwrap();
    let layer = send0(unsafe { objc_getClass(c"CAMetalLayer".as_ptr()) }, c"new");
    let source = SurfaceSource::Layer(Arc::new(Layer(NonNull::new(layer).unwrap())));
    let display = page.layout(390, 844, 1.0).unwrap();
    let id = surfaces::attach(source, 390, 844, 0).unwrap();
    surfaces::prepare(id, &display, 1).unwrap();
    surfaces::present(id, 1).unwrap();

    surfaces::trim(id);
    assert_eq!(
        drawable_size(layer),
        Size {
            width: 390.0,
            height: 844.0
        },
        "an uncovered Metal surface keeps its size"
    );
    surfaces::trim_covered(id);
    assert_eq!(
        drawable_size(layer),
        Size {
            width: 1.0,
            height: 1.0
        },
        "parked"
    );
    assert!(
        surfaces::readback(id).unwrap().is_none(),
        "the render targets went"
    );

    surfaces::prepare(id, &display, 2).unwrap();
    surfaces::present(id, 2).unwrap();
    assert_eq!(
        drawable_size(layer),
        Size {
            width: 390.0,
            height: 844.0
        }
    );
    assert_eq!(pixel(id, 195, 800), (390, 844, [0x3b, 0x82, 0xf6, 0xff]));

    surfaces::suspend(id);
    surfaces::detach(id);
    send0(layer, c"release");
}
