//! Automation requests (selectors and actions) against one presented page.
#![no_main]

use lingxia_ui::interaction::frame::PresentedFrame;
use lingxia_ui::interaction::view::{AutomationAct, ViewInteraction};
use lingxia_ui::session::lifecycle::{CommitTicket, PageTicket};
use serde_json::Value;
use std::sync::{Arc, LazyLock};

static PAGE: LazyLock<lingxia_ui::DisplayList> = LazyLock::new(|| {
    let wire: Vec<Value> = serde_json::from_str(include_str!("../page.json")).unwrap();
    let records = lingxia_ui::protocol::decode(&wire).unwrap();
    lingxia_ui::Renderer::default()
        .layout(&records, 320, 480, 1.0)
        .unwrap()
});

fn interaction() -> ViewInteraction {
    let mut interaction = ViewInteraction::default();
    interaction.presented(
        Arc::new(PresentedFrame::new(
            CommitTicket::new(1),
            1.0,
            PAGE.nodes.clone(),
        )),
        0,
        &Default::default(),
        PageTicket::new(1),
    );
    interaction
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // A JSON request as the driver sends it, else the bytes as one selector.
    let request: Value = serde_json::from_slice(data).unwrap_or_else(
        |_| serde_json::json!({"op": "query", "css": String::from_utf8_lossy(data)}),
    );
    let css = request["css"].as_str().unwrap_or_default();
    let index = request["index"].as_u64().map(|i| i as usize);
    let mut interaction = interaction();
    match request["op"].as_str() {
        Some("act") => {
            let action = &request["action"];
            let text = |key: &str| action[key].as_str().unwrap_or_default().to_owned();
            let act = match action["kind"].as_str() {
                Some("fill") => AutomationAct::Fill(text("text")),
                Some("type") => AutomationAct::Type(text("text")),
                Some("scroll") => AutomationAct::ScrollIntoView,
                Some("press") => AutomationAct::Press(text("key")),
                Some("select") => AutomationAct::SelectOption(text("value")),
                Some("dblclick") => AutomationAct::DoubleClick,
                _ => AutomationAct::Click,
            };
            let force = action["force"].as_bool().unwrap_or(false);
            let _ = interaction.automate(css, index, act, force, PageTicket::new(1));
        }
        Some("tree") => {
            let _ = interaction.automation_tree();
        }
        _ => {
            let all = request["all"].as_bool().unwrap_or(false);
            let max = request["maxText"].as_u64().map(|n| n as usize);
            let _ = interaction.automation_query(css, index, all, max);
        }
    }
});
