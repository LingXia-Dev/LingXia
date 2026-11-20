//! Metal memory a page's surface holds must come back. A binary of its own: the device-wide
//! allocated size would count other tests' GPU work.
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

fn allocated_mib(device: *mut c_void) -> f64 {
    send0(device, c"currentAllocatedSize") as usize as f64 / 1_048_576.0
}

fn new_layer() -> *mut c_void {
    send0(unsafe { objc_getClass(c"CAMetalLayer".as_ptr()) }, c"new")
}

fn source(layer: *mut c_void) -> SurfaceSource {
    SurfaceSource::Layer(Arc::new(Layer(NonNull::new(layer).unwrap())))
}

/// Every surface call here runs on threads without a run loop, as the render thread does.
#[test]
fn page_surfaces_hand_back_their_metal_memory() {
    let device = unsafe { MTLCreateSystemDefaultDevice() };
    if device.is_null() {
        return;
    }
    let (width, height) = (390u16, 844u16);
    let module = format!(
        "{}\nfunction render(){{return h('view',{{style:{{flexGrow:1,backgroundColor:'#3b82f6'}}}},h('text',{{style:{{fontSize:20}}}},'Page'))}}\n__lingxiaPublish(render);",
        include_str!("../src/session/tests/runtime.js")
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(width, height, 1.0).unwrap();
    let mut revision = 1;

    // Each relaunched page binds a fresh layer; one kept per page adds up.
    let mut relaunch = || {
        let layer = new_layer();
        let id = surfaces::attach(source(layer), width.into(), height.into(), 0).unwrap();
        for _ in 0..3 {
            surfaces::prepare(id, &display, revision).unwrap();
            let _ = surfaces::present(id, revision);
            revision += 1;
        }
        std::thread::spawn(move || {
            surfaces::suspend(id);
            surfaces::detach(id);
        })
        .join()
        .unwrap();
        let held = send0(layer, c"retainCount") as usize;
        send0(layer, c"release");
        held
    };
    relaunch();
    let settled = allocated_mib(device);
    for _ in 0..10 {
        assert_eq!(relaunch(), 1, "a dropped page's layer is still retained");
    }
    let grown = allocated_mib(device) - settled;
    assert!(
        grown < 2.0,
        "10 relaunched pages kept {grown:.1} MiB of Metal memory"
    );

    // A page left visible for many frames hands everything back once released: what a frame
    // left in an open transaction would outlive the page. In-page pools vary with GPU load.
    let before = allocated_mib(device);
    let layer = new_layer();
    let id = surfaces::attach(source(layer), width.into(), height.into(), 0).unwrap();
    for _ in 0..500 {
        surfaces::prepare(id, &display, revision).unwrap();
        let _ = surfaces::present(id, revision);
        revision += 1;
    }
    surfaces::suspend(id);
    surfaces::detach(id);
    assert_eq!(
        send0(layer, c"retainCount") as usize,
        1,
        "a page released after 500 frames keeps its layer"
    );
    send0(layer, c"release");
    let kept = allocated_mib(device) - before;
    assert!(
        kept < 2.0,
        "a page released after 500 frames kept {kept:.1} MiB of Metal memory"
    );

    // Released while nothing else draws: the app sits idle on the page below, so no later
    // submit would free the dropped page's targets.
    let (width, height) = (2048u16, 2048u16);
    let display = page.layout(width, height, 1.0).unwrap();
    let before = allocated_mib(device);
    let layer = new_layer();
    let id = surfaces::attach(source(layer), width.into(), height.into(), 0).unwrap();
    for _ in 0..3 {
        surfaces::prepare(id, &display, revision).unwrap();
        let _ = surfaces::present(id, revision);
        revision += 1;
    }
    surfaces::suspend(id);
    surfaces::detach(id);
    send0(layer, c"release");
    let kept = allocated_mib(device) - before;
    assert!(
        kept < 2.0,
        "a large page released with nothing drawing after it kept {kept:.1} MiB of Metal memory"
    );
}
