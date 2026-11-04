//! Automation queries over the semantic projection: the CSS subset test
//! locators emit, matched against a virtual element per node, answered in
//! the same JSON shape as a WebView DOM query.
use super::projection::{Projection, Role, SemanticNode};
use crate::scene::node::NodeSnapshot;
use serde_json::{Value, json};

/// Selector forms GPU pages answer; anything else is rejected by name.
pub const SUPPORTED: &str = "tag names (button, a, input, textarea, img, h1-h6, ul, li, dialog, div, span), [data-testid=\"…\"], [data-*] and [data-*=\"…\"] (authored data-* props), [role=\"…\"], [aria-label=\"…\"], [placeholder=\"…\"], [type=\"…\"], [href], comma lists of these";

#[derive(Debug, PartialEq)]
struct Simple {
    tag: Option<String>,
    attributes: Vec<(String, Option<String>)>,
}

fn unsupported(selector: &str) -> String {
    format!("E_SELECTOR_UNSUPPORTED: `{selector}` on a GPU page; supported: {SUPPORTED}")
}

fn simple(source: &str) -> Result<Simple, String> {
    let source = source.trim();
    let tag_end = source.find('[').unwrap_or(source.len());
    let tag = &source[..tag_end];
    if !tag.chars().all(|c| c.is_ascii_alphanumeric())
        || (tag.is_empty() && tag_end == source.len())
    {
        return Err(unsupported(source));
    }
    let mut attributes = Vec::new();
    let mut rest = &source[tag_end..];
    while !rest.is_empty() {
        let close = rest.find(']').ok_or_else(|| unsupported(source))?;
        let body = rest.strip_prefix('[').ok_or_else(|| unsupported(source))?[..close - 1].trim();
        let (name, value) = match body.split_once('=') {
            Some((name, value)) => {
                let value = value.trim();
                let unquoted = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                    .unwrap_or(value);
                (
                    name.trim(),
                    Some(unquoted.replace("\\\"", "\"").replace("\\\\", "\\")),
                )
            }
            None => (body, None),
        };
        if !matches!(
            name,
            "role" | "aria-label" | "placeholder" | "type" | "href"
        ) && !data_name(name)
        {
            return Err(unsupported(source));
        }
        attributes.push((name.to_owned(), value));
        rest = &rest[close + 1..];
    }
    Ok(Simple {
        tag: (!tag.is_empty()).then(|| tag.to_ascii_lowercase()),
        attributes,
    })
}

/// `data-testid` and the authored `data-*` props.
fn data_name(name: &str) -> bool {
    name.strip_prefix("data-").is_some_and(|rest| {
        rest.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

fn parse(selector: &str) -> Result<Vec<Simple>, String> {
    if selector.trim().is_empty() {
        return Err("selector must not be empty".into());
    }
    let mut parts = Vec::new();
    let (mut start, mut quote, mut escaped) = (0, None, false);
    for (at, c) in selector.char_indices() {
        match (quote, c) {
            _ if escaped => escaped = false,
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, ',') => {
                parts.push(simple(&selector[start..at])?);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(simple(&selector[start..])?);
    Ok(parts)
}

/// The HTML-like element a GPU node answers to, so one locator works on both renderers.
struct Virtual {
    tag: String,
    role: Option<&'static str>,
    kind: Option<&'static str>,
    link: bool,
}

fn aria(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Button => "button",
        Role::CheckBox => "checkbox",
        Role::Switch => "switch",
        Role::RadioGroup => "radiogroup",
        Role::RadioButton => "radio",
        Role::ComboBox => "combobox",
        Role::TextField => "textbox",
        Role::Image => "img",
        Role::Slider => "slider",
        Role::Link => "link",
        Role::Heading => "heading",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::Dialog => "dialog",
        Role::Alert => "alert",
        Role::Status => "status",
        Role::Grid => "grid",
        Role::GridCell => "gridcell",
        Role::ListBox => "listbox",
        Role::Option => "option",
        Role::Menu => "menu",
        Role::MenuItem => "menuitem",
        Role::Text | Role::ScrollView | Role::HorizontalScrollView => return None,
    })
}

fn virtual_element(node: &NodeSnapshot, semantic: Option<&SemanticNode>) -> Virtual {
    let role = semantic.map(|s| s.role);
    let (tag, kind) = match node.tag {
        "button" => ("button", None),
        "link" => ("a", None),
        "input" => (
            "input",
            Some(if node.input.as_ref().is_some_and(|i| i.secure) {
                "password"
            } else {
                "text"
            }),
        ),
        "search" => ("input", Some("search")),
        "textarea" => ("textarea", None),
        "checkbox" => ("input", Some("checkbox")),
        "radio-option" => ("input", Some("radio")),
        "slider" => ("input", Some("range")),
        "image" => ("img", None),
        "select" => ("select", None),
        _ => match role {
            Some(Role::Heading) => ("h", None),
            Some(Role::List) => ("ul", None),
            Some(Role::ListItem) => ("li", None),
            Some(Role::Dialog) => ("dialog", None),
            _ if node.tag == "text" || node.tag == "span" => ("span", None),
            _ => ("div", None),
        },
    };
    let tag = if tag == "h" {
        format!("h{}", semantic.and_then(|s| s.heading_level).unwrap_or(2))
    } else {
        tag.to_owned()
    };
    Virtual {
        tag,
        role: role.and_then(aria),
        kind,
        link: node.tag == "link",
    }
}

fn matches(simple: &Simple, element: &Virtual, node: &NodeSnapshot, name: &str) -> bool {
    simple.tag.as_ref().is_none_or(|tag| *tag == element.tag)
        && simple.attributes.iter().all(|(attribute, value)| {
            let actual = match attribute.as_str() {
                "data-testid" => (!node.test_id.is_empty()).then_some(node.test_id.as_str()),
                "role" => element.role,
                "aria-label" => (!name.is_empty()).then_some(name),
                "type" => element.kind,
                "href" => element.link.then_some(""),
                "placeholder" => node
                    .input
                    .as_ref()
                    .map(|input| input.placeholder.as_str())
                    .filter(|placeholder| !placeholder.is_empty()),
                data => node
                    .data
                    .as_ref()
                    .and_then(|map| map.get(&data[5..]))
                    .map(String::as_str),
            };
            match (actual, value) {
                (Some(actual), Some(value)) => actual == value,
                (Some(_), None) => true,
                (None, _) => false,
            }
        })
}

fn truncate(text: &str, max: Option<usize>) -> (String, bool) {
    match max {
        Some(max) if text.chars().count() > max => (text.chars().take(max).collect(), true),
        _ => (text.to_owned(), false),
    }
}

/// Matching node indices in reading (source) order. A test id matches hidden
/// nodes too, so tests can assert that something is hidden.
pub fn select(projection: &Projection, selector: &str) -> Result<Vec<usize>, String> {
    let selectors = parse(selector)?;
    Ok(projection
        .frame
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let semantic = projection.ids.id(*index).and_then(|id| projection.node(id));
            let element = virtual_element(node, semantic.as_ref());
            let name = semantic.as_ref().map(|s| s.name.as_str()).unwrap_or("");
            let reachable = semantic.is_some()
                || selectors
                    .iter()
                    .all(|s| s.attributes.iter().any(|(a, _)| a == "data-testid"));
            reachable && selectors.iter().any(|s| matches(s, &element, node, name))
        })
        .map(|(index, _)| index)
        .collect())
}

/// What a DOM element's rendered text reads: its own text, else its rendered descendants'
/// text in order, one space apart. Editor values are values, not text.
fn text_content(nodes: &[NodeSnapshot], index: usize) -> String {
    let node = &nodes[index];
    if node.input.is_none() && !node.text.is_empty() {
        return node.text.clone();
    }
    let prefix = format!("{}/", node.identity);
    let mut parts: Vec<&str> = Vec::new();
    let mut covered: Option<String> = None;
    for child in nodes[index + 1..]
        .iter()
        .take_while(|child| child.identity.starts_with(&prefix))
    {
        // A text node's runs are its text already.
        if covered
            .as_ref()
            .is_some_and(|text| child.identity.starts_with(text.as_str()))
        {
            continue;
        }
        if child.input.is_none()
            && !child.text.is_empty()
            && child.bounds[2] > 0.0
            && child.bounds[3] > 0.0
        {
            parts.push(child.text.as_str());
            covered = Some(format!("{}/", child.identity));
        }
    }
    parts.join(" ")
}

/// The node's `data-testid` and authored `data-*` props, as DOM attributes.
fn attributes(node: &NodeSnapshot) -> Value {
    let mut out = serde_json::Map::new();
    if !node.test_id.is_empty() {
        out.insert("data-testid".into(), json!(node.test_id));
    }
    for (name, value) in node.data.iter().flat_map(|map| map.iter()) {
        out.insert(format!("data-{name}"), json!(value));
    }
    Value::Object(out)
}

/// One match in the WebView query payload shape.
pub fn describe(
    projection: &Projection,
    index: usize,
    position: usize,
    count: usize,
    max_text: Option<usize>,
) -> Value {
    let node = &projection.frame.nodes[index];
    let semantic = projection.ids.id(index).and_then(|id| projection.node(id));
    let element = virtual_element(node, semantic.as_ref());
    // As on the Web, visible means rendered; scrolled or clipped away is only out of the viewport.
    let visible = node.bounds[2] > 0.0 && node.bounds[3] > 0.0;
    let [left, top, width, height] = node.visible_bounds;
    let viewport = projection
        .frame
        .nodes
        .first()
        .map(|root| [root.bounds[2], root.bounds[3]])
        .unwrap_or_default();
    let in_viewport = visible
        && width > 0.0
        && height > 0.0
        && left + width > 0.0
        && top + height > 0.0
        && left < viewport[0]
        && top < viewport[1];
    let input = node.input.as_ref();
    let chosen = node.select.as_ref().and_then(|select| {
        select
            .selected
            .map(|index| select.options[index].value.clone())
    });
    let value = chosen
        .or_else(|| {
            input.map(|input| {
                input
                    .edit
                    .as_ref()
                    .map(|edit| edit.text.clone())
                    .unwrap_or_else(|| input.value.clone())
            })
        })
        // A range reads as `<input type="range">` does: its number as text.
        .or_else(|| {
            semantic
                .as_ref()
                .and_then(|s| s.range.as_ref())
                .map(|range| range.value.to_string())
        });
    let (text, text_truncated) = truncate(&text_content(&projection.frame.nodes, index), max_text);
    let value = value.map(|value| truncate(&value, max_text));
    let [left, top, width, height] = node.bounds;
    json!({
        "exists": true,
        "renderer": "gpu",
        "index": position,
        "count": count,
        "tag": element.tag,
        "type": element.kind,
        "id": Value::Null,
        "name": Value::Null,
        "role": element.role,
        "aria_label": semantic.as_ref().map(|s| s.name.clone()).filter(|n| !n.is_empty()),
        "placeholder": input.map(|i| i.placeholder.clone()).filter(|p| !p.is_empty()),
        "visible": visible,
        "inViewport": in_viewport,
        "enabled": semantic.as_ref().is_none_or(|s| s.enabled),
        "editable": input.is_some_and(|i| i.enabled),
        "focused": semantic.as_ref().is_some_and(|s| s.focused),
        "checked": node.checked,
        "text": text,
        "text_truncated": text_truncated,
        "value": value.as_ref().map(|(v, _)| v.clone()),
        "value_truncated": value.is_some_and(|(_, t)| t),
        "attributes": attributes(node),
        "rect": {
            "left": left, "top": top, "width": width, "height": height,
            "right": left + width, "bottom": top + height,
            "center_x": left + width / 2.0, "center_y": top + height / 2.0,
            "viewport_width": viewport[0], "viewport_height": viewport[1],
        },
    })
}

/// The accessibility tree a screen reader walks, in reading order: role, name,
/// test id, state and bounds per node, nested under their containers.
pub fn tree(projection: &Projection) -> Value {
    fn entry(projection: &Projection, id: u32, depth: usize) -> Option<Value> {
        let node = projection.node(id)?;
        let role = aria(node.role).map(str::to_owned).unwrap_or_else(|| {
            serde_json::to_value(node.role)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        });
        let mut out = serde_json::Map::new();
        out.insert("role".into(), json!(role));
        out.insert("name".into(), json!(node.name));
        if !node.test_id.is_empty() {
            out.insert("testId".into(), json!(node.test_id));
        }
        out.insert("bounds".into(), json!(node.bounds));
        if let Some(drag) = &projection.frame.nodes[node.index].drag {
            let index = projection
                .drags
                .and_then(|drags| drags.target(&projection.frame.nodes[node.index].identity))
                .unwrap_or(drag.index);
            let axis = if drag.axis == crate::scene::node::ScrollAxis::Horizontal {
                "x"
            } else {
                "y"
            };
            out.insert(
                "drag".into(),
                json!({ "axis": axis, "index": index, "count": drag.points.len() }),
            );
        }
        for (key, flag) in [
            ("disabled", !node.enabled),
            ("focused", node.focused),
            ("offscreen", !node.visible),
        ] {
            if flag {
                out.insert(key.into(), json!(true));
            }
        }
        for (key, state) in [
            ("checked", node.checked),
            ("pressed", node.pressed),
            ("selected", node.selected),
            ("expanded", node.expanded),
            ("busy", node.busy),
        ] {
            if let Some(state) = state {
                out.insert(key.into(), json!(state));
            }
        }
        if let Some(level) = node.heading_level {
            out.insert("level".into(), json!(level));
        }
        if let Some(field) = &node.text_field {
            out.insert("value".into(), json!(field.text));
        } else if let Some(range) = &node.range {
            out.insert("value".into(), json!(range.value));
        }
        if let Some(scroll) = node.scroll {
            out.insert("scroll".into(), json!([scroll.offset, scroll.max_offset]));
        }
        // Bounded so a pathological page cannot recurse the dump without end.
        let children: Vec<Value> = if depth < 64 {
            node.children
                .iter()
                .filter_map(|child| entry(projection, *child, depth + 1))
                .collect()
        } else {
            Vec::new()
        };
        if !children.is_empty() {
            out.insert("children".into(), Value::Array(children));
        }
        Some(Value::Object(out))
    }
    let roots: Vec<Value> = projection
        .roots()
        .into_iter()
        .filter_map(|id| entry(projection, id, 0))
        .collect();
    json!({ "renderer": "gpu", "nodes": roots })
}

/// The DOM query contract: one match (`index`, default the first) or `all`.
pub fn query(
    projection: &Projection,
    selector: &str,
    index: Option<usize>,
    all: bool,
    max_text: Option<usize>,
) -> Result<Value, String> {
    let matches = select(projection, selector)?;
    let count = matches.len();
    if all {
        let items: Vec<_> = matches
            .iter()
            .enumerate()
            .map(|(position, node)| describe(projection, *node, position, count, max_text))
            .collect();
        return Ok(json!({ "count": count, "renderer": "gpu", "items": items }));
    }
    let position = index.unwrap_or(0);
    Ok(match matches.get(position) {
        Some(node) => describe(projection, *node, position, count, max_text),
        None => json!({
            "exists": false, "renderer": "gpu", "index": position, "count": count,
            "visible": false, "inViewport": false, "enabled": false, "editable": false,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_locator_subset_and_names_the_rest() {
        assert_eq!(parse("button, [role=\"button\"]").unwrap().len(), 2);
        assert_eq!(
            parse("[data-testid=\"save \\\"now\\\"\"]").unwrap()[0].attributes[0]
                .1
                .as_deref(),
            Some("save \"now\"")
        );
        assert_eq!(
            parse("input[type=\"text\"]").unwrap()[0].tag.as_deref(),
            Some("input")
        );
        assert!(parse("a[href]").is_ok());
        assert_eq!(
            parse("[aria-label=\"Pay, now\"], button").unwrap()[0].attributes[0]
                .1
                .as_deref(),
            Some("Pay, now")
        );
        for selector in [
            "div > span",
            ".card",
            "#main",
            "li:first-child",
            "[aria-checked=true]",
            "[data-State]",
            "*",
        ] {
            assert!(
                parse(selector)
                    .unwrap_err()
                    .starts_with("E_SELECTOR_UNSUPPORTED"),
                "{selector}"
            );
        }
    }
}
