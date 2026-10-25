//! Which records may nest, carry inline text, or carry a field on a tag.
use super::{FIELDS, Record, field, tag};
use serde_json::Value;

const EDITORS: [usize; 3] = [tag::INPUT, tag::TEXTAREA, tag::SEARCH];

/// A child of `parent` with `child` tag may be appended.
pub(super) fn child(records: &[Record], parent: usize, child: usize) -> Result<(), String> {
    if matches!(
        records[parent].tag,
        tag::TEXT
            | tag::SPACER
            | tag::IMAGE
            | tag::INPUT
            | tag::TEXTAREA
            | tag::SWITCH
            | tag::RADIO_OPTION
            | tag::SEARCH
            | tag::SLIDER
            | tag::PLOT
            | tag::VECTOR
    ) {
        return Err("UI_CHILD: leaf node".into());
    }
    if matches!(records[parent].tag, tag::BUTTON | tag::CHECKBOX)
        && !records[parent].text.is_empty()
    {
        return Err("UI_BUTTON: a control with children carries no inline text".into());
    }
    let mut ancestor = Some(parent);
    while let Some(owner) = ancestor {
        if matches!(records[owner].tag, tag::BUTTON | tag::CHECKBOX)
            && !matches!(
                child,
                tag::COLUMN
                    | tag::ROW
                    | tag::TEXT
                    | tag::SPACER
                    | tag::IMAGE
                    | tag::GRID
                    | tag::STACK
                    | tag::VECTOR
            )
        {
            return Err("UI_BUTTON: descendants must be noninteractive content".into());
        }
        ancestor = records[owner].parent;
    }
    Ok(())
}

pub(super) fn text(tag: usize, text: &str) -> Result<(), String> {
    if !matches!(
        tag,
        tag::TEXT | tag::BUTTON | tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION
    ) && !text.is_empty()
    {
        return Err("UI_TEXT: container".into());
    }
    Ok(())
}

fn on(allowed: bool, error: &str) -> Result<(), String> {
    if allowed { Ok(()) } else { Err(error.into()) }
}

/// Field `id` applies to a record of `tag` under a parent of `parent` tag.
pub(super) fn field(tag: usize, parent: Option<usize>, id: usize) -> Result<(), String> {
    let editor = EDITORS.contains(&tag);
    match id {
        field::FLEX_DIRECTION | field::DISPLAY => on(
            matches!(tag, tag::COLUMN | tag::ROW | tag::PAGE),
            "UI_LAYOUT: flex direction/display requires a flex container",
        ),
        field::SCROLL_AXIS => on(
            tag == tag::SCROLL,
            "UI_SCROLL: scrollAxis requires a scroll node",
        ),
        field::LIST_ITEM_KEYS => on(tag == tag::LIST, "UI_LIST: itemKeys requires a list"),
        field::FOCUS_STYLE => on(editor, "UI_STYLE: focus styles require an editor"),
        field::VECTOR_SOURCE => on(tag == tag::VECTOR, "UI_VECTOR: source requires vector node"),
        field::SUBMIT_ON_ENTER => on(tag == tag::TEXTAREA, "UI_INPUT: invalid submitOnEnter"),
        field::PLOT_MARKER_EMPHASIS_SCALE => {
            on(tag == tag::PLOT, "UI_PLOT: invalid marker emphasis scale")
        }
        field::PLOT_PIE_LAYOUT => on(tag == tag::PLOT, "UI_PLOT: invalid pie layout"),
        field::PLOT_DOMAIN_MIN | field::PLOT_DOMAIN_MAX => {
            on(tag == tag::PLOT, "UI_PLOT: invalid domain bound")
        }
        field::ON_PLOT_SELECT => on(tag == tag::PLOT, "UI_PLOT: invalid selection callback"),
        field::PLOT_SELECTED_INDEX => on(tag == tag::PLOT, "UI_PLOT: invalid selected index"),
        field::PLOT_SMOOTH | field::PLOT_AREA => {
            on(tag == tag::PLOT, "UI_PLOT: invalid boolean property")
        }
        field::PLOT_VALUES | field::PLOT_KIND | field::PLOT_COLORS => {
            on(tag == tag::PLOT, "UI_PLOT: invalid encoded property")
        }
        field::TEXT_LINES => on(
            tag == tag::TEXT,
            "UI_TEXT: lines must be an integer between 1 and 1024 on text",
        ),
        field::GRID_COLUMNS => on(
            tag == tag::GRID,
            "UI_GRID: columns must be an integer between 1 and 64 on a grid",
        ),
        field::SLIDER_VALUE
        | field::SLIDER_MIN
        | field::SLIDER_MAX
        | field::SLIDER_STEP
        | field::ON_SLIDER_CHANGE => on(tag == tag::SLIDER, "UI_SLIDER: property on another tag"),
        field::IMAGE_RETRY_KEY => on(tag == tag::IMAGE, "UI_IMAGE: invalid retry key"),
        field::ON_IMAGE_LOAD | field::ON_IMAGE_ERROR => {
            on(tag == tag::IMAGE, "UI_IMAGE: invalid callback")
        }
        field::IMAGE_ALT => on(tag == tag::IMAGE, "UI_IMAGE: invalid alternative text"),
        field::ACCESSIBILITY_LABEL => on(
            matches!(
                tag,
                tag::BUTTON
                    | tag::INPUT
                    | tag::TEXTAREA
                    | tag::CHECKBOX
                    | tag::SWITCH
                    | tag::RADIO
                    | tag::SEARCH
                    | tag::SLIDER
                    | tag::PLOT
            ),
            "UI_PROP: invalid accessibility label",
        ),
        field::CHECKED | field::ON_CHECKED_CHANGE => on(
            matches!(tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION),
            "UI_CHECK: property on another tag",
        ),
        field::INPUT_VALUE
        | field::PLACEHOLDER
        | field::AUTO_FOCUS
        | field::KEYBOARD
        | field::ON_INPUT
        | field::ON_SUBMIT => on(editor, "UI_LIST: property on another tag"),
        field::SECURE => on(
            matches!(tag, tag::INPUT | tag::TEXTAREA),
            "UI_LIST: property on another tag",
        ),
        field::IMAGE_SRC | field::IMAGE_FIT => {
            on(tag == tag::IMAGE, "UI_LIST: property on another tag")
        }
        field::ON_REACH_START | field::ON_REACH_END => on(
            matches!(tag, tag::SCROLL | tag::LIST),
            "UI_LIST: property on another tag",
        ),
        field::LIST_ID
        | field::LIST_COUNT
        | field::LIST_LEADING
        | field::LIST_TRAILING
        | field::LIST_ANCHOR => on(tag == tag::LIST, "UI_LIST: property on another tag"),
        field::LIST_ROW_INDEX => on(
            tag == tag::COLUMN && parent == Some(tag::LIST),
            "UI_LIST: property on another tag",
        ),
        field::ON_CLICK => on(
            tag == tag::BUTTON,
            "UI_PROP: button property on another tag",
        ),
        field::ENABLED => on(
            matches!(
                tag,
                tag::BUTTON
                    | tag::INPUT
                    | tag::TEXTAREA
                    | tag::CHECKBOX
                    | tag::SWITCH
                    | tag::RADIO
                    | tag::RADIO_OPTION
                    | tag::SEARCH
                    | tag::SLIDER
            ),
            "UI_PROP: button property on another tag",
        ),
        field::PRESSED_STYLE => on(
            matches!(tag, tag::BUTTON | tag::CHECKBOX | tag::SWITCH | tag::RADIO),
            "UI_STYLE: pressed styles require a control",
        ),
        _ if FIELDS[id].1 == "reserved" => {
            Err(format!("UI_FIELD: {} is not accepted", FIELDS[id].0))
        }
        _ => Ok(()),
    }
}

fn has(fields: &[(usize, Value)], id: usize) -> bool {
    fields.iter().any(|(field, _)| *field == id)
}

/// Field combinations and placement a complete record must satisfy.
pub(super) fn record(
    tag: usize,
    parent: Option<usize>,
    key: &str,
    fields: &[(usize, Value)],
) -> Result<(), String> {
    for (absolute, percent) in super::DIMENSIONS {
        if has(fields, absolute) && has(fields, percent) {
            return Err("UI_STYLE: conflicting absolute and percentage dimensions".into());
        }
    }
    if has(fields, field::HEIGHT_AUTO)
        && (has(fields, field::HEIGHT) || has(fields, field::HEIGHT_PERCENT))
    {
        return Err("UI_STYLE: conflicting automatic height".into());
    }
    if tag == tag::PAGE && parent.is_some() {
        return Err("UI_PAGE: page must be the root".into());
    }
    if (tag == tag::RADIO_OPTION) != (parent == Some(tag::RADIO)) {
        return Err("UI_RADIO: options require a radio parent, groups accept only options".into());
    }
    if tag == tag::RADIO_OPTION && (key.is_empty() || !has(fields, field::CHECKED)) {
        return Err("UI_RADIO: option requires a key and checked flag".into());
    }
    if tag == tag::GRID && !has(fields, field::GRID_COLUMNS) {
        return Err("UI_GRID: missing columns".into());
    }
    if tag == tag::IMAGE && !has(fields, field::IMAGE_SRC) {
        return Err("UI_IMAGE: missing source".into());
    }
    if tag == tag::LIST
        && ![
            field::LIST_ID,
            field::LIST_COUNT,
            field::LIST_LEADING,
            field::LIST_TRAILING,
        ]
        .iter()
        .all(|required| has(fields, *required))
    {
        return Err("UI_LIST: missing geometry".into());
    }
    if parent == Some(tag::LIST) && !has(fields, field::LIST_ROW_INDEX) {
        return Err("UI_LIST: expected row wrapper".into());
    }
    Ok(())
}
