//! A page's DirectComposition tree. Its frames show above the page's child windows, so a
//! `<webview>` surface window under a transparent cutout shows through, as under a Metal layer.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionClip, IDCompositionDesktopDevice, IDCompositionTarget,
    IDCompositionVisual, IDCompositionVisual2,
};
use windows::core::Interface;

pub(crate) struct Tree {
    device: IDCompositionDesktopDevice,
    _target: IDCompositionTarget,
    _root: IDCompositionVisual2,
    frames: IDCompositionVisual2,
    /// Held across each change and commit: the render thread commits a new swapchain while the
    /// page's thread clips the frames.
    busy: Mutex<()>,
}

// SAFETY: DirectComposition objects are not bound to a thread; `busy` serializes their use.
unsafe impl Send for Tree {}
unsafe impl Sync for Tree {}

/// Each page window's tree.
static TREES: LazyLock<Mutex<HashMap<isize, Arc<Tree>>>> = LazyLock::new(Default::default);

fn trees() -> MutexGuard<'static, HashMap<isize, Arc<Tree>>> {
    TREES.lock().unwrap_or_else(|p| p.into_inner())
}

/// `page`'s tree, made on first use on the page's thread; `None` where DirectComposition is
/// unavailable.
pub(crate) fn tree(page: isize) -> Option<Arc<Tree>> {
    if let Some(tree) = existing(page) {
        return Some(tree);
    }
    let tree = match Tree::new(page) {
        Ok(tree) => Arc::new(tree),
        Err(error) => {
            log::warn!(target: "lingxia_view", "page composition unavailable: {error}");
            return None;
        }
    };
    trees().insert(page, tree.clone());
    Some(tree)
}

pub(crate) fn existing(page: isize) -> Option<Arc<Tree>> {
    trees().get(&page).cloned()
}

pub(crate) fn forget(page: isize) {
    trees().remove(&page);
}

/// Frames stop above `top` (window pixels), where a child window's panel shows; `None` shows
/// them whole.
pub(crate) fn clip_above(page: isize, top: Option<f32>) {
    let Some(tree) = existing(page) else {
        return;
    };
    let _busy = tree.busy.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: plain DirectComposition calls on live objects.
    let _ = unsafe {
        match top {
            Some(top) => tree.frames.SetClip2(&D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: f32::MAX,
                bottom: top,
            }),
            None => tree.frames.SetClip(None::<&IDCompositionClip>),
        }
    };
    tree.commit_now();
}

impl Tree {
    fn new(page: isize) -> windows::core::Result<Self> {
        // SAFETY: plain DirectComposition calls; the target lives no longer than the window
        // it is made for, which drops the tree when it goes.
        unsafe {
            let device: IDCompositionDesktopDevice =
                DCompositionCreateDevice2(None::<&windows::core::IUnknown>)?;
            let target = device.CreateTargetForHwnd(HWND(page as *mut _), true)?;
            let root = device.CreateVisual()?;
            let frames = device.CreateVisual()?;
            root.AddVisual(&frames, false, None::<&IDCompositionVisual>)?;
            target.SetRoot(&root)?;
            device.Commit()?;
            Ok(Self {
                device,
                _target: target,
                _root: root,
                frames,
                busy: Mutex::new(()),
            })
        }
    }

    fn commit_now(&self) {
        // SAFETY: commits this tree's own device.
        if let Err(error) = unsafe { self.device.Commit() } {
            log::warn!(target: "lingxia_view", "page composition commit: {error}");
        }
    }
}

// SAFETY: the visual is owned by this value's composition tree.
unsafe impl lingxia_view::renderer::gpu::CompositionVisual for Tree {
    fn visual(&self) -> std::ptr::NonNull<std::ffi::c_void> {
        std::ptr::NonNull::new(self.frames.as_raw()).expect("a live visual")
    }

    fn commit(&self) {
        let _busy = self.busy.lock().unwrap_or_else(|p| p.into_inner());
        self.commit_now();
    }
}
