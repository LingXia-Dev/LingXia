use std::sync::Arc;

/// One pull-down menu of the Windows application menu bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAppMenu {
    pub title: String,
    pub entries: Vec<WindowsAppMenuEntry>,
}

impl WindowsAppMenu {
    /// Creates a pull-down menu with the given title and entries.
    pub fn new(
        title: impl Into<String>,
        entries: impl IntoIterator<Item = WindowsAppMenuEntry>,
    ) -> Self {
        Self {
            title: title.into(),
            entries: entries.into_iter().collect(),
        }
    }

    /// Appends one entry and returns the updated menu.
    pub fn with_entry(mut self, entry: WindowsAppMenuEntry) -> Self {
        self.entries.push(entry);
        self
    }
}

/// One entry in a Windows application menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsAppMenuEntry {
    Item(WindowsAppMenuItem),
    Separator,
}

impl WindowsAppMenuEntry {
    /// Creates a selectable menu item entry.
    pub fn item(item: WindowsAppMenuItem) -> Self {
        Self::Item(item)
    }

    /// Creates a separator entry.
    pub fn separator() -> Self {
        Self::Separator
    }
}

impl From<WindowsAppMenuItem> for WindowsAppMenuEntry {
    fn from(value: WindowsAppMenuItem) -> Self {
        Self::Item(value)
    }
}

/// A selectable Windows application-menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAppMenuItem {
    pub id: u32,
    pub label: String,
    pub checked: bool,
    pub accelerator_vk: Option<u32>,
}

impl WindowsAppMenuItem {
    /// Creates a selectable menu item with a caller-owned command id.
    ///
    /// Command ids must be non-zero and no larger than `0xFFFF`; the Windows
    /// menu path reports them through the low word of `WM_COMMAND`.
    pub fn new(id: u32, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            checked: false,
            accelerator_vk: None,
        }
    }

    /// Sets whether the item is drawn with a check mark.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Binds a plain virtual-key accelerator such as `0x7B` for F12.
    pub fn accelerator_vk(mut self, vk: u32) -> Self {
        self.accelerator_vk = Some(vk);
        self
    }
}

pub type WindowsAppMenuCommandHandler = Arc<dyn Fn(u32) + Send + Sync>;
