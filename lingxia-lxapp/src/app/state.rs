use std::path::{Path, PathBuf};

pub(crate) const APP_STATE_DIR: &str = "app_state";

pub fn dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(APP_STATE_DIR)
}

pub fn file(app_data_dir: &Path, name: &str) -> PathBuf {
    dir(app_data_dir).join(name)
}
