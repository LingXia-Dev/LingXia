/// Browser-owned management navigation. These routes are intentionally
/// separate from the product-wide static Settings destination resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowserLocalNavigation<'a> {
    Settings,
    ClearSiteData { tab_id: &'a str },
}

impl BrowserLocalNavigation<'_> {
    pub(crate) fn page(&self) -> &'static str {
        match self {
            Self::Settings | Self::ClearSiteData { .. } => "settings",
        }
    }

    pub(crate) fn query(&self) -> Option<serde_json::Value> {
        match self {
            Self::Settings => None,
            Self::ClearSiteData { tab_id } => Some(serde_json::json!({
                "section": "clear-site-data",
                "tabId": tab_id,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_local_settings_and_clear_site_data_target_control_surface_pages() {
        assert_eq!(BrowserLocalNavigation::Settings.page(), "settings");
        assert_eq!(BrowserLocalNavigation::Settings.query(), None);
        let clear = BrowserLocalNavigation::ClearSiteData { tab_id: "tab-1" };
        assert_eq!(clear.page(), "settings");
        assert_eq!(
            clear.query(),
            Some(serde_json::json!({ "section": "clear-site-data", "tabId": "tab-1" }))
        );
    }
}
