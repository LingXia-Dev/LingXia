//! Public host-window layout token.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsPanelPosition {
    Left,
    #[default]
    Right,
    Bottom,
}

/// Opaque layout payload stored by the WebView host and handed back to the
/// registered chrome renderer.
///
/// The WebView layer intentionally does not define navigation bars, tab
/// bars, address bars, or other shell chrome concepts. A host layer owns
/// its strongly typed layout model and stores it here with [`Self::new`];
/// its renderer can recover that type with [`Self::downcast_ref`].
#[derive(Clone, Default)]
pub struct WindowsWindowLayout {
    payload: Option<Arc<dyn std::any::Any + Send + Sync>>,
}

impl std::fmt::Debug for WindowsWindowLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsWindowLayout")
            .field("has_payload", &self.payload.is_some())
            .finish()
    }
}

impl WindowsWindowLayout {
    pub fn new<T>(payload: T) -> Self
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        Self {
            payload: Some(Arc::new(payload)),
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_none()
    }

    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: std::any::Any + 'static,
    {
        self.payload.as_deref()?.downcast_ref::<T>()
    }
}
