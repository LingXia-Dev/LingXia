use crate::{ActivatorCollection, ActivatorDeclaration, PinCollection, ShellError, ShellResult};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

pub const ACTIVATOR_STORE_FILE: &str = "shell-activators-v1.json";
pub const PIN_STORE_FILE: &str = "shell-pins-v1.json";

#[derive(Debug, Clone)]
pub struct ShellStore {
    root: PathBuf,
}

impl ShellStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load_activators(&self) -> ShellResult<ActivatorCollection> {
        let Some(value) = self.load_optional::<ActivatorDeclaration>(ACTIVATOR_STORE_FILE)? else {
            return Ok(ActivatorCollection::default());
        };
        ActivatorCollection::restore(value)
    }

    pub fn save_activators(&self, activators: &ActivatorCollection) -> ShellResult<()> {
        self.save(ACTIVATOR_STORE_FILE, &activators.declaration())
    }

    pub fn load_pins(&self) -> ShellResult<PinCollection> {
        let Some(value) = self.load_optional::<PinCollection>(PIN_STORE_FILE)? else {
            return Ok(PinCollection::default());
        };
        value.restore()
    }

    pub fn save_pins(&self, pins: &PinCollection) -> ShellResult<()> {
        self.save(PIN_STORE_FILE, pins)
    }

    fn load_optional<T: DeserializeOwned>(&self, name: &str) -> ShellResult<Option<T>> {
        let path = self.root.join(name);
        if !path.is_file() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .map_err(|error| ShellError::Io(format!("read {}: {error}", path.display())))?;
        serde_json::from_str(&raw)
            .map(Some)
            .map_err(|error| ShellError::InvalidState(format!("{}: {error}", path.display())))
    }

    fn save<T: Serialize>(&self, name: &str, value: &T) -> ShellResult<()> {
        fs::create_dir_all(&self.root)
            .map_err(|error| ShellError::Io(format!("create {}: {error}", self.root.display())))?;
        let path = self.root.join(name);
        let tmp = self.root.join(format!("{name}.tmp"));
        let raw = serde_json::to_vec_pretty(value)?;
        fs::write(&tmp, raw)
            .map_err(|error| ShellError::Io(format!("write {}: {error}", tmp.display())))?;
        replace_file(&tmp, &path)
    }
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, path: &Path) -> ShellResult<()> {
    fs::rename(tmp, path)
        .map_err(|error| ShellError::Io(format!("replace {}: {error}", path.display())))
}

#[cfg(windows)]
fn replace_file(tmp: &Path, path: &Path) -> ShellResult<()> {
    let backup = path.with_extension("json.bak");
    if backup.exists() {
        fs::remove_file(&backup)
            .map_err(|error| ShellError::Io(format!("remove {}: {error}", backup.display())))?;
    }
    let had_previous = path.exists();
    if had_previous {
        fs::rename(path, &backup)
            .map_err(|error| ShellError::Io(format!("backup {}: {error}", path.display())))?;
    }
    if let Err(error) = fs::rename(tmp, path) {
        if had_previous {
            let _ = fs::rename(&backup, path);
        }
        return Err(ShellError::Io(format!(
            "replace {}: {error}",
            path.display()
        )));
    }
    if had_previous {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ShellActivator, ShellActivatorTarget, ShellPinTarget};

    #[test]
    fn stores_explicit_empty_activators_and_mixed_pins_independently() {
        let dir = tempfile::tempdir().unwrap();
        let store = ShellStore::new(dir.path());
        let mut activators = ActivatorCollection::default();
        activators.clear();
        store.save_activators(&activators).unwrap();

        let mut pins = PinCollection::default();
        pins.pin(ShellPinTarget::Lxapp {
            key: "app.chat".to_string(),
        })
        .unwrap();
        pins.pin(ShellPinTarget::Bookmark {
            key: "bookmark-a".to_string(),
        })
        .unwrap();
        store.save_pins(&pins).unwrap();

        let restored_activators = store.load_activators().unwrap();
        let restored_pins = store.load_pins().unwrap();
        assert!(restored_activators.declared());
        assert!(restored_activators.items().is_empty());
        assert_eq!(restored_pins, pins);
    }

    #[test]
    fn action_activators_are_not_restored() {
        let dir = tempfile::tempdir().unwrap();
        let store = ShellStore::new(dir.path());
        let mut activators = ActivatorCollection::default();
        activators
            .replace(vec![ShellActivator {
                id: "sync".to_string(),
                target: ShellActivatorTarget::Action,
                label: Some("Sync".to_string()),
                icon: Some("icons/sync.svg".to_string()),
                disabled: false,
            }])
            .unwrap();
        store.save_activators(&activators).unwrap();

        let restored = store.load_activators().unwrap();
        assert!(restored.declared());
        assert!(restored.items().is_empty());
    }
}
