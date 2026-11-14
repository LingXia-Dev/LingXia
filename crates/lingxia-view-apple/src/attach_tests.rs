use super::*;
use lingxia_view::renderer::surfaces;
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};

struct Quiet;

impl Shell for Quiet {
    fn present_pixels(&self, _: i64, _: &[u8], _: u32, _: u32) -> bool {
        false
    }
    fn loaded(&self, _: i64) {}
    fn request_vsync(&self, _: i64) -> bool {
        false
    }
    fn arm_long_press(&self, _: i64) {}
    fn cancel_long_press(&self, _: i64) {}
    fn fling(&self, _: i64, _: &str, _: bool, _: f32) {}
    fn stop_fling(&self, _: i64) {}
    fn feedback(&self, _: i64, _: bool, _: &[f32], _: u32, _: f32) {}
    fn haptic(&self, _: i64) {}
    fn top_layer(&self, _: i64, _: bool) {}
    fn video_orientation(&self, _: i64, _: Option<lingxia_view::scene::video::Orientation>) {}
    fn claim(&self, _: i64, _: bool) {}
    fn copy_text(&self, _: &str) {}
    fn text_menu(&self, _: i64, _: &str) {}
    fn host_chrome(&self, _: i64, _: &lingxia_view::platform::HostChrome) {}
    fn ime_presented(&self, _: i64, _: u64, _: &str) {}
    fn ime_focus(&self, _: i64, _: u64, _: &str, _: Option<[f32; 2]>, _: bool) {}
    fn ime_activation(&self, _: i64) {}
    fn ime_reset(&self, _: i64) {}
    fn ime_edit(
        &self,
        _: i64,
        _: u64,
        _: &str,
        _: Option<&str>,
        _: i32,
        _: i32,
    ) -> lingxia_view::platform::EditOutcome {
        lingxia_view::platform::EditOutcome::Failed
    }
    fn accessibility_event(&self, _: i64, _: u8, _: u32, _: &str) {}
    fn presented(&self, _: i64, _: u64) {}
    fn logic(&self, _: i64, _: u64, _: Vec<lingxia_view::session::logic::ToLogic>) {}
    fn state(&self, _: i64, _: u64, _: lingxia_view::session::state::StateSignal) {}
    fn restart(&self, _: i64, _: u64) -> bool {
        false
    }
    fn recovery(&self, _: i64, _: Option<&Failure>) {}
    fn bootstrap_overdue(&self, _: i64, _: bool) {}
    fn load_image(
        &self,
        _: i64,
        _: String,
        _: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send> {
        Box::new(())
    }
}

#[link(name = "QuartzCore", kind = "framework")]
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_msgSend();
    fn sel_registerName(name: *const c_char) -> *const c_void;
    fn objc_getClass(name: *const c_char) -> *const c_void;
    fn objc_allocateClassPair(
        superclass: *const c_void,
        name: *const c_char,
        extra: usize,
    ) -> *mut c_void;
    fn objc_registerClassPair(class: *mut c_void);
    fn class_addMethod(
        class: *mut c_void,
        name: *const c_void,
        imp: *const c_void,
        types: *const c_char,
    ) -> bool;
}

static LAYER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

extern "C" fn layer(_: *mut c_void, _: *const c_void) -> *mut c_void {
    LAYER.load(Ordering::Acquire)
}

/// A stand-in for the Swift page view: an object whose `layer` is a CAMetalLayer.
fn metal_view() -> usize {
    unsafe {
        let send: unsafe extern "C" fn(*const c_void, *const c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let metal = send(
            objc_getClass(c"CAMetalLayer".as_ptr()),
            sel_registerName(c"new".as_ptr()),
        );
        LAYER.store(metal, Ordering::Release);
        let class = objc_allocateClassPair(
            objc_getClass(c"NSObject".as_ptr()),
            c"LxAttachTestView".as_ptr(),
            0,
        );
        class_addMethod(
            class,
            sel_registerName(c"layer".as_ptr()),
            layer as *const c_void,
            c"@@:".as_ptr(),
        );
        objc_registerClassPair(class);
        send(class, sel_registerName(c"new".as_ptr())) as usize
    }
}

/// Presenters in the registry; ids start at 1 and this process makes only a few.
fn presenters() -> usize {
    render::run_now(|| {
        (1..64)
            .filter(|id| surfaces::describe(*id).is_some())
            .count()
    })
    .unwrap()
}

/// A page closed before its attach reaches the main thread must not leave its presenter
/// registered. A test thread never drains the main queue, so the attach's post never runs.
#[test]
fn a_page_dropped_before_its_attach_lands_leaves_no_presenter() {
    let page = Page::new(1, Arc::new(Quiet), "attach-test", false).unwrap();
    page.attach(metal_view(), 64, 64);
    // `run_now` goes ahead of queued work, so wait for the queued attach itself.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while presenters() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(
        presenters(),
        1,
        "the attach made a presenter (needs a Metal device)"
    );
    drop(page);
    assert_eq!(
        presenters(),
        0,
        "the dropped page's presenter is still registered"
    );
}
