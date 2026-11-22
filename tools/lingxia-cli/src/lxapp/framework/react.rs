use super::FrameworkScaffold;

const INDEX_HTML_TEMPLATE: &str =
    include_str!("../../../templates/builder-frameworks/react/index.html");
const MAIN_TEMPLATE: &str = include_str!("../../../templates/builder-frameworks/react/main.jsx");

pub(super) fn scaffold(
    page_title: &str,
    app_import: &str,
    page_bridge_import: &str,
    messages_import: Option<&str>,
    wait_for_state: bool,
) -> FrameworkScaffold {
    let (messages_import, messages) = match messages_import {
        Some(path) => (format!("import * as messages from '{path}';"), "messages"),
        None => (String::new(), "messages: undefined"),
    };
    FrameworkScaffold {
        index_html: INDEX_HTML_TEMPLATE.replace(
            "<title>LingXia React Page</title>",
            &format!("<title>{page_title}</title>"),
        ),
        main_entry_filename: "main.jsx",
        main_entry: MAIN_TEMPLATE
            .replace("/* {{APP_IMPORT}} */", app_import)
            .replace(
                "/* {{PAGE_BRIDGE_IMPORT}} */",
                &format!(
                    "{page_bridge_import}
{messages_import}"
                ),
            )
            .replace("/* {{MESSAGES}} */", messages)
            .replace(
                "/* {{WAIT_FOR_STATE}} */",
                if wait_for_state { "true" } else { "false" },
            ),
        output_extension: ".tsx",
    }
}
