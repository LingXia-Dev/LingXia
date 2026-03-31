use super::FrameworkScaffold;

pub(super) fn scaffold(
    page_title: &str,
    _app_import: &str,
    _page_bridge_import: &str,
) -> FrameworkScaffold {
    FrameworkScaffold {
        index_html: format!(
            "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no\"><title>{page_title}</title></head><body></body></html>"
        ),
        main_entry_filename: "main.js",
        main_entry: String::new(),
        output_extension: ".html",
    }
}
