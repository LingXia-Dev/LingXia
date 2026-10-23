/// Records a browser page's console in the page; see `page_console.js`. The
/// outer scope keeps the shared formatter off `window`.
pub(crate) const PAGE_CONSOLE_RECORDER: &str = concat!(
    "(function () {\n",
    include_str!("console_format.js"),
    include_str!("page_console.js"),
    "})();\n"
);
