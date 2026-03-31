use super::FrameworkScaffold;

const INDEX_HTML_TEMPLATE: &str =
    include_str!("../../../templates/builder-frameworks/react/index.html");
const MAIN_TEMPLATE: &str = include_str!("../../../templates/builder-frameworks/react/main.jsx");

pub(super) fn scaffold(
    page_title: &str,
    app_import: &str,
    page_bridge_import: &str,
) -> FrameworkScaffold {
    FrameworkScaffold {
        index_html: INDEX_HTML_TEMPLATE.replace(
            "<title>LingXia React Page</title>",
            &format!("<title>{page_title}</title>"),
        ),
        main_entry_filename: "main.jsx",
        main_entry: MAIN_TEMPLATE
            .replace("/* {{APP_IMPORT}} */", app_import)
            .replace("/* {{PAGE_BRIDGE_IMPORT}} */", page_bridge_import),
        output_extension: ".tsx",
    }
}
