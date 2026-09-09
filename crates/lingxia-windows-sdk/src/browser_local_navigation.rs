/// Browser-owned management navigation. These routes are intentionally
/// separate from the product-wide static Settings destination resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowserLocalNavigation<'a> {
    Settings,
    ClearSiteData { tab_id: &'a str },
}

impl BrowserLocalNavigation<'_> {
    pub(crate) fn url(&self) -> String {
        match self {
            Self::Settings => "lingxia://settings".to_string(),
            Self::ClearSiteData { tab_id } => {
                format!("lingxia://settings#clear-site-data?tabId={tab_id}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_local_settings_and_clear_site_data_keep_private_routes() {
        assert_eq!(BrowserLocalNavigation::Settings.url(), "lingxia://settings");
        assert_eq!(
            BrowserLocalNavigation::ClearSiteData { tab_id: "tab-1" }.url(),
            "lingxia://settings#clear-site-data?tabId=tab-1"
        );
    }
}
