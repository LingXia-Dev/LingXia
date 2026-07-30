//! Provider-owned metadata projected into platform surface switchers.

use serde::{Deserialize, Serialize};

use crate::SurfaceContent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum SurfaceIcon {
    BuiltIn { name: String },
    Resource { uri: String },
    ProviderAsset { provider: String, key: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceCapabilities {
    pub close: bool,
    pub rename: bool,
}

impl Default for SurfaceCapabilities {
    fn default() -> Self {
        Self {
            close: true,
            rename: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacePresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<SurfaceIcon>,
    pub capabilities: SurfaceCapabilities,
}

impl SurfacePresentation {
    pub fn for_content(content: &SurfaceContent) -> Self {
        match content {
            SurfaceContent::Lxapp { app_id, .. } => Self {
                automatic_title: Some(app_id.clone()),
                custom_title: None,
                icon: Some(SurfaceIcon::ProviderAsset {
                    provider: "lxapp".into(),
                    key: app_id.clone(),
                }),
                capabilities: SurfaceCapabilities {
                    close: false,
                    rename: false,
                },
            },
            SurfaceContent::Page { path, .. } => Self {
                automatic_title: Some(path.clone()),
                custom_title: None,
                icon: None,
                capabilities: SurfaceCapabilities::default(),
            },
            SurfaceContent::Browser { initial_url, .. } => Self {
                automatic_title: Some(initial_url.clone()),
                custom_title: None,
                icon: Some(SurfaceIcon::BuiltIn {
                    name: "browser".into(),
                }),
                capabilities: SurfaceCapabilities::default(),
            },
            SurfaceContent::Native { capability } => Self {
                automatic_title: Some(capability.clone()),
                custom_title: None,
                icon: Some(SurfaceIcon::BuiltIn {
                    name: capability.clone(),
                }),
                capabilities: SurfaceCapabilities::default(),
            },
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.custom_title
            .as_deref()
            .or(self.automatic_title.as_deref())
    }

    pub fn set_custom_title(&mut self, title: Option<&str>) {
        self.custom_title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string);
    }
}
