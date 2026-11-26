//! Theme steps resolve against the page's scales and follow a `scales` push without the View.
use super::*;
use crate::protocol::SCALES;
use crate::protocol::scales::Scales;
use serde_json::{Value, json};

fn slot(name: &str) -> usize {
    SCALES.iter().position(|(slot, _, _)| *slot == name).unwrap()
}

fn env(scales: Option<Value>, text_scale: f64) -> Value {
    let mut value = json!({
        "appearance": "light",
        "insets": {"top": 0, "right": 0, "bottom": 0, "left": 0},
        "keyboard": {"height": 0},
        "pointer": "coarse",
        "os": "unknown",
        "language": "en",
        "canDock": false,
        "textScale": [[0.0, 0.0], [16384.0, 16384.0 * text_scale]],
    });
    if let Some(scales) = scales {
        value["scales"] = scales;
    }
    value
}

fn bounds(frame: &crate::DisplayList, id: &str) -> [f32; 4] {
    frame.nodes.iter().find(|n| n.test_id == id).unwrap().bounds
}

/// Presents frame `index` (`true`: the frame the motion starts at): its nodes and wakeup.
type Present<'a> = dyn FnMut(
        &mut ViewSession,
        u64,
        bool,
    ) -> (
        Vec<crate::scene::node::NodeSnapshot>,
        crate::animation::Wakeup,
    ) + 'a;

#[test]
fn a_scales_push_resolves_steps_again_without_running_the_view() {
    let (space, text, leading) = (slot("space"), slot("text.base"), slot("leading.base"));
    let mut page = ViewSession::new(&source(&format!(
        "function render(){{return h('view',{{'data-testid':'box',style:{{paddingTop:[{space},4,0],paddingLeft:[{space},2,0],gap:[{space},3,0]}}}},\
         h('text',{{'data-testid':'a',style:{{fontSize:[{text},1,0],lineHeight:[{leading},1,0]}}}},'A'),\
         h('view',{{'data-testid':'b',style:{{height:10,marginLeft:[{space},-1,0]}}}}))}}"
    )))
    .unwrap();
    page.set_environment(env(None, 1.0)).unwrap();
    let frame = page.layout(300, 300, 1.0).unwrap();
    let (boxed, a, b) = (
        bounds(&frame, "box"),
        bounds(&frame, "a"),
        bounds(&frame, "b"),
    );
    assert_eq!(a[1] - boxed[1], 16.0, "p-4 under a 4-point unit");
    assert_eq!(a[0] - boxed[0], 8.0);
    assert_eq!(a[3], 22.0, "the pack's base line height");
    assert_eq!(b[1] - (a[1] + a[3]), 12.0, "gap-3");
    assert_eq!(b[0] - boxed[0], 4.0, "-ml-1 inside pl-2");

    let mut pushed = Scales::builtin().push();
    pushed["revision"] = 1.into();
    pushed["values"][space] = 8.0.into();
    pushed["values"][leading] = 44.0.into();
    page.set_environment(env(Some(pushed), 1.0)).unwrap();
    let frame = page.prepare_native(300, 300, 1.0, false, None, Ok).unwrap();
    assert_eq!(page.frame_metrics().view_passes, 0, "the View does not run");
    let (boxed, a, b) = (
        bounds(&frame, "box"),
        bounds(&frame, "a"),
        bounds(&frame, "b"),
    );
    assert_eq!(a[1] - boxed[1], 32.0);
    assert_eq!(a[0] - boxed[0], 16.0);
    assert_eq!(a[3], 44.0);
    assert_eq!(b[1] - (a[1] + a[3]), 24.0);
    assert_eq!(b[0] - boxed[0], 8.0);

    // Back to the built-in pack: a push the host stops sending.
    page.set_environment(env(None, 1.0)).unwrap();
    let frame = page.prepare_native(300, 300, 1.0, false, None, Ok).unwrap();
    assert_eq!(bounds(&frame, "a")[1] - bounds(&frame, "box")[1], 16.0);
}

#[test]
fn a_scales_push_that_puts_a_size_below_zero_is_refused() {
    let space = slot("space");
    let mut page = ViewSession::new(&source(&format!(
        "function render(){{return h('view',{{style:{{paddingTop:[{space},2,-6]}}}})}}"
    )))
    .unwrap();
    page.set_environment(env(None, 1.0)).unwrap();
    page.layout(100, 100, 1.0).unwrap();
    let mut pushed = Scales::builtin().push();
    pushed["revision"] = 1.into();
    pushed["values"][space] = 2.0.into();
    assert_eq!(
        page.set_environment(env(Some(pushed), 1.0)).unwrap_err(),
        "UI_STYLE: paddingTop resolves below 0"
    );
    let mut wrong = Scales::builtin().push();
    wrong["values"][slot("ease.enter")] = 1.into();
    assert!(page.set_environment(env(Some(wrong), 1.0)).is_err());
}

#[test]
fn an_em_size_grows_with_the_text_up_to_one_and_a_half_times() {
    let (text, em) = (slot("text.sm"), slot("em"));
    // Ten em, so whole points: text-sm is 13 points, 16.9 at a 1.3 ratio, capped at 19.5.
    let mut page = ViewSession::new(&source(&format!(
        "function render(){{return h('view',{{style:{{fontSize:[{text},1,0]}}}},\
         h('view',{{'data-testid':'icon',style:{{width:[{em},10,0],height:[{em},10,2]}}}}))}}"
    )))
    .unwrap();
    for (ratio, side) in [(1.0, 130.0), (1.3, 169.0), (2.0, 195.0)] {
        page.set_environment(env(None, ratio)).unwrap();
        let icon = bounds(&page.layout(300, 300, 1.0).unwrap(), "icon");
        assert_eq!([icon[2], icon[3]], [side, side + 2.0], "ratio {ratio}");
    }
}

#[test]
fn em_spacing_follows_the_text_and_a_margin_keeps_its_sign() {
    let (text, em) = (slot("text.sm"), slot("em"));
    // text-sm is 13 points: 1em padding 13, 2em gap 26, -1em margin -13; ×1.3 at a 1.3 ratio.
    let mut page = ViewSession::new(&source(&format!(
        "function render(){{return h('view',{{'data-testid':'box',style:{{fontSize:[{text},1,0],paddingLeft:[{em},1,0],paddingTop:[{em},1,1],gap:[{em},2,0]}}}},\
         h('view',{{'data-testid':'a',style:{{height:10}}}}),\
         h('view',{{'data-testid':'b',style:{{height:10,marginLeft:[{em},-1,0]}}}}))}}"
    )))
    .unwrap();
    for (ratio, em) in [(1.0, 13.0), (1.3, 16.9)] {
        page.set_environment(env(None, ratio)).unwrap();
        let frame = page.layout(300, 300, 1.0).unwrap();
        let (a, b) = (bounds(&frame, "a"), bounds(&frame, "b"));
        // Boxes land on whole pixels.
        let close = |x: f32, y: f32| (x - y).abs() <= 1.0;
        assert!(
            close(a[0], em) && close(a[1], em + 1.0),
            "padding, ratio {ratio}: {a:?}"
        );
        assert!(
            close(b[1] - (a[1] + 10.0), 2.0 * em),
            "gap, ratio {ratio}: {b:?}"
        );
        assert!(
            close(b[0], 0.0),
            "margin pulls back by 1em, ratio {ratio}: {b:?}"
        );
    }
}

#[test]
fn min_and_max_size_keywords_size_by_content() {
    let mut page = ViewSession::new(&source(
        "function render(){return h('view',{style:{width:60,flexDirection:'row'}},\
         h('text',{'data-testid':'word',style:{minWidth:'min-content',flexShrink:1}},'Unbreakable'),\
         h('view',{style:{width:100}}))}",
    ))
    .unwrap();
    page.set_environment(env(None, 1.0)).unwrap();
    let frame = page.layout(300, 300, 1.0).unwrap();
    let word = bounds(&frame, "word");
    assert!(word[2] > 40.0, "min-w-min keeps the longest word: {word:?}");

    let mut page = ViewSession::new(&source(
        "function render(){return h('view',{style:{width:200}},\
         h('view',{'data-testid':'capped',style:{maxWidth:'max-content'}},h('text',{},'Hi')))}",
    ))
    .unwrap();
    page.set_environment(env(None, 1.0)).unwrap();
    let capped = bounds(&page.layout(300, 300, 1.0).unwrap(), "capped");
    assert!(
        capped[2] < 60.0,
        "max-w-max caps at the content: {capped:?}"
    );
}

#[test]
fn a_running_transition_keeps_its_length_across_a_scales_push() {
    use crate::animation::Wakeup;
    use crate::session::lifecycle::SurfaceTicket;
    use std::time::Duration;
    const SURFACE: SurfaceTicket = SurfaceTicket::new(1);
    let at = |index: u64| Duration::from_micros(index * 16_667);
    let base = slot("duration.base");
    let mut page = ViewSession::new(&source(&format!(
        "function render(){{const open=useLocal('open',false);return h('page',{{}},\
         h('button',{{'data-testid':'toggle',onClick(){{open.set(!open.value)}},style:{{width:40,height:10}}}},'T'),\
         h('view',{{'data-testid':'panel',style:{{width:100,height:open.value?80:20,\
         transition:{{properties:['size'],durationMs:[{base},1,0],delayMs:0,easing:'linear'}}}}}}))}}"
    )))
    .unwrap();
    page.set_environment(env(None, 1.0)).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let mut present = |page: &mut ViewSession, index: u64, rebuild: bool| {
        let nodes = page
            .prepare_view_animated(
                200,
                100,
                1.0,
                rebuild,
                None,
                at(index),
                |d: crate::DisplayList| Ok(d.nodes.clone()),
            )
            .unwrap();
        assert!(
            page.animation_presented(SURFACE, page.generation(), at(index))
                .unwrap()
        );
        (nodes, page.view_animation_wakeup(at(index)).unwrap())
    };
    let toggle = |page: &mut ViewSession, nodes: &[crate::scene::node::NodeSnapshot]| {
        let handle = nodes
            .iter()
            .find(|n| n.test_id == "toggle")
            .unwrap()
            .on_click
            .unwrap();
        assert!(page.click(page.generation(), handle).unwrap());
    };
    // Frames until the motion that the frame at `from` starts settles.
    let settle = |page: &mut ViewSession, from: u64, present: &mut Present| {
        (from..from + 120)
            .find(|&index| present(page, index, index == from).1 == Wakeup::None)
            .expect("the transition never ends")
            - from
    };

    let (nodes, _) = present(&mut page, 0, true);
    toggle(&mut page, &nodes);
    for index in 1..=4 {
        present(&mut page, index, index == 1);
    }
    let mut pushed = Scales::builtin().push();
    pushed["revision"] = 1.into();
    pushed["values"][base] = 1000.0.into();
    page.set_environment(env(Some(pushed), 1.0)).unwrap();
    let rest = settle(&mut page, 5, &mut present);
    assert!(
        (4..=12).contains(&rest),
        "the 200 ms run keeps its length: {rest} more frames"
    );

    let (nodes, _) = present(&mut page, 40, true);
    toggle(&mut page, &nodes);
    let next = settle(&mut page, 41, &mut present);
    assert!(
        next >= 55,
        "the next run takes the pushed 1000 ms: {next} frames"
    );
}

#[test]
fn an_em_step_reaches_a_sticky_inset_and_a_list_gap() {
    let (text, em) = (slot("text.sm"), slot("em"));
    // text-sm is 13 points: a sticky header 1em down, list rows 2em apart.
    let mut page = ViewSession::new(&source(&format!(
        "function render(){{return h('view',{{style:{{fontSize:[{text},1,0]}}}},\
         h('scroll',{{'data-testid':'s',style:{{height:100}}}},h('text',{{'data-testid':'h',style:{{position:'sticky',top:[{em},1,0],height:20}}}},'H'),h('view',{{style:{{height:400}}}})),\
         h(List,{{'data-testid':'list',style:{{height:120,gap:[{em},2,0]}},items:[1,2,3],getKey:i=>String(i),renderItem:i=>h('text',{{style:{{height:20}}}},String(i))}}))}}"
    )))
    .unwrap();
    page.set_environment(env(None, 1.0)).unwrap();
    let frame = page.layout(300, 300, 1.0).unwrap();
    let id = frame
        .nodes
        .iter()
        .find(|n| n.test_id == "s")
        .and_then(|n| n.scroll.as_ref())
        .unwrap()
        .id
        .clone();
    let scrolled = frame
        .retained
        .compose(&std::collections::HashMap::from([(id, 200.0)]))
        .unwrap();
    let (s, h) = (bounds(&scrolled, "s"), bounds(&scrolled, "h"));
    assert!(
        (h[1] - (s[1] + 13.0)).abs() <= 1.0,
        "stuck 1em below the top: {h:?} {s:?}"
    );
    let gap = frame
        .nodes
        .iter()
        .find_map(|n| n.scroll.as_ref().filter(|_| n.test_id == "list"))
        .unwrap()
        .row_gap;
    assert!(
        (gap - 26.0).abs() <= 1.0,
        "the list measures rows 2em apart: {gap}"
    );
}
