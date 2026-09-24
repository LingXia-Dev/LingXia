use super::FrameworkScaffold;

const INDEX_HTML_TEMPLATE: &str =
    include_str!("../../../templates/builder-frameworks/vue/index.html");
const MAIN_TEMPLATE: &str = include_str!("../../../templates/builder-frameworks/vue/main.js");

pub(super) fn scaffold(
    page_title: &str,
    app_import: &str,
    page_bridge_import: &str,
    wait_for_state: bool,
) -> FrameworkScaffold {
    FrameworkScaffold {
        index_html: INDEX_HTML_TEMPLATE.replace(
            "<title>LingXia Vue Page</title>",
            &format!("<title>{page_title}</title>"),
        ),
        main_entry_filename: "main.js",
        main_entry: MAIN_TEMPLATE
            .replace("/* {{APP_IMPORT}} */", app_import)
            .replace("/* {{PAGE_BRIDGE_IMPORT}} */", page_bridge_import)
            .replace("/* {{WAIT_FOR_STATE}} */", if wait_for_state { "true" } else { "false" }),
        output_extension: ".vue",
    }
}
