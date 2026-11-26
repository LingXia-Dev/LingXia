//! Presence on a fake 60 Hz clock: `starting:` enters, `exit:` leaves as a ghost.
use crate::session::lifecycle::SurfaceTicket;
use crate::{DisplayList, ViewSession, animation::Wakeup};
use std::time::Duration;

const SURFACE: SurfaceTicket = SurfaceTicket::new(1);

fn frame_time(index: u64) -> Duration {
    Duration::from_micros(index * 16_667)
}

/// A toggle button and, while `shown`, a black 50x20 panel entering from and leaving to opacity 0.
fn page(shown: bool, scroll: bool) -> ViewSession {
    let panel = "h('view',{'data-testid':'panel',className:'starting:fade exit:fade',style:{width:50,height:20,backgroundColor:'#000000',transition:{properties:['opacity'],durationMs:200,delayMs:0,easing:'linear'}}})";
    let content = if scroll {
        format!(
            "h('scroll',{{'data-testid':'s',style:{{height:30,width:100}}}},h('view',{{style:{{height:300,paddingTop:20}}}},shown.value && {panel}))"
        )
    } else {
        format!("shown.value && {panel}")
    };
    let module = super::source(&format!(
        "function render(){{const shown=useLocal('shown',{shown});return h('page',{{style:{{backgroundColor:'#ffffff'}}}},h('button',{{'data-testid':'toggle',onClick(){{shown.set(!shown.value)}},style:{{position:'absolute',left:150,top:0,width:40,height:20}}}},'T'),{content})}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'starting:fade',when:{state:'starting'},style:{opacity:0}},{name:'exit:fade',when:{state:'exit'},style:{opacity:0}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    page
}

struct Shot {
    nodes: Vec<crate::scene::node::NodeSnapshot>,
    pixels: Vec<u8>,
}

fn present(page: &mut ViewSession, at: Duration, rebuild: bool) -> (Shot, Wakeup) {
    let shot = page
        .prepare_view_animated(200, 100, 1.0, rebuild, None, at, |display: DisplayList| {
            let nodes = display.nodes.clone();
            let pixels = display
                .rasterize()
                .unwrap()
                .pixels
                .data_as_u8_slice()
                .to_vec();
            Ok(Shot { nodes, pixels })
        })
        .unwrap();
    assert!(
        page.animation_presented(SURFACE, page.generation(), at)
            .unwrap()
    );
    (shot, page.view_animation_wakeup(at).unwrap())
}

/// The red channel at the panel's top-left area: 255 is white page, 0 a fully drawn panel.
fn ink(shot: &Shot) -> u8 {
    shot.pixels[(10 * 200 + 10) * 4]
}

/// Present from frame 0 until nothing is scheduled; returns the last frame and its index.
fn settle(page: &mut ViewSession) -> (Shot, u64) {
    let (mut shot, mut wakeup) = present(page, frame_time(0), true);
    let mut index = 0;
    while wakeup != Wakeup::None && index < 60 {
        index += 1;
        (shot, wakeup) = present(page, frame_time(index), false);
    }
    (shot, index)
}

fn toggle(page: &mut ViewSession, shot: &Shot) {
    let handle = shot
        .nodes
        .iter()
        .find(|n| n.test_id == "toggle")
        .unwrap()
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), handle).unwrap());
}

#[test]
fn a_starting_panel_enters_from_its_starting_values() {
    let mut page = page(false, false);
    let (shot, _) = present(&mut page, frame_time(0), true);
    assert_eq!(ink(&shot), 255);
    toggle(&mut page, &shot);
    let mut samples = Vec::new();
    for index in 1..30 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == 1);
        samples.push(ink(&shot));
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert_eq!(samples[0], 255, "the first frame shows the starting value");
    assert_eq!(*samples.last().unwrap(), 0, "it lands on its own style");
    assert!(samples.windows(2).all(|w| w[0] >= w[1]), "{samples:?}");
    assert!(
        samples.iter().filter(|v| (1..255).contains(*v)).count() >= 8,
        "{samples:?}"
    );
}

#[test]
fn an_exiting_panel_leaves_as_a_ghost_that_takes_no_input() {
    let mut page = page(true, false);
    let (shot, start) = settle(&mut page);
    assert_eq!(ink(&shot), 0);
    toggle(&mut page, &shot);
    let mut samples = Vec::new();
    for index in start + 1..start + 30 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == start + 1);
        samples.push(ink(&shot));
        assert!(
            shot.nodes.iter().all(|n| n.test_id != "panel"),
            "a ghost has no test id"
        );
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert_eq!(samples[0], 0, "the ghost starts from the panel as it was");
    assert_eq!(
        *samples.last().unwrap(),
        255,
        "and is gone when the exit ends"
    );
    assert!(samples.windows(2).all(|w| w[0] <= w[1]), "{samples:?}");
    assert!(
        samples.iter().filter(|v| (1..255).contains(*v)).count() >= 8,
        "{samples:?}"
    );
}

#[test]
fn reduced_motion_lands_enters_and_exits_at_once() {
    let mut page = page(true, false);
    page.set_environment(serde_json::json!({"appearance":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en","canDock":false,"reducedMotion":true})).unwrap();
    let (shot, _) = present(&mut page, frame_time(0), true);
    toggle(&mut page, &shot);
    let (shot, wakeup) = present(&mut page, frame_time(1), true);
    assert_eq!(
        (ink(&shot), wakeup),
        (255, Wakeup::None),
        "no ghost under reduced motion"
    );
}

#[test]
fn a_ghost_scrolls_and_clips_with_its_scroller() {
    // The panel sits 20..40 inside a 30-high scroller: its lower half is clipped away.
    let mut page = page(true, true);
    let (shot, start) = settle(&mut page);
    toggle(&mut page, &shot);
    let (shot, _) = present(&mut page, frame_time(start + 1), true);
    let ghost = shot
        .nodes
        .iter()
        .find(|n| n.test_id.is_empty() && n.bounds[2] == 50.0 && n.bounds[3] == 20.0)
        .expect("the ghost is drawn");
    assert_eq!(ghost.accessibility_hidden, Some(true));
    assert_eq!(ghost.on_click, None);
    assert!(
        ghost.visible_bounds[1] + ghost.visible_bounds[3] <= 30.0 + 1e-3,
        "clipped by its scroller: {:?}",
        ghost.visible_bounds
    );
    assert!(ghost.visible_bounds[3] > 0.0, "still drawn inside it");
}

fn panels(shot: &Shot) -> usize {
    shot.nodes
        .iter()
        .filter(|n| n.bounds[2] == 50.0 && n.bounds[3] == 20.0)
        .count()
}

#[test]
fn the_ghost_count_reaches_frame_metrics() {
    let mut page = page(true, false);
    let (shot, start) = settle(&mut page);
    assert_eq!(page.frame_metrics().ghosts, 0);
    toggle(&mut page, &shot);
    present(&mut page, frame_time(start + 1), true);
    assert_eq!(page.frame_metrics().ghosts, 1);
}

#[test]
fn a_hidden_page_drops_its_ghosts() {
    let mut page = page(true, false);
    let (shot, start) = settle(&mut page);
    page.set_animation_target(Some(SURFACE), false, frame_time(start))
        .unwrap();
    toggle(&mut page, &shot);
    let at = frame_time(start + 1);
    let shot = page
        .prepare_view_animated(200, 100, 1.0, true, None, at, |display: DisplayList| {
            Ok(Shot {
                nodes: display.nodes.clone(),
                pixels: display
                    .rasterize()
                    .unwrap()
                    .pixels
                    .data_as_u8_slice()
                    .to_vec(),
            })
        })
        .unwrap();
    assert_eq!((ink(&shot), panels(&shot)), (255, 0));
    page.animation_presented(SURFACE, page.generation(), at)
        .unwrap();
    page.set_animation_target(Some(SURFACE), true, at).unwrap();
    let (shot, wakeup) = present(&mut page, frame_time(start + 2), true);
    assert_eq!((ink(&shot), panels(&shot), wakeup), (255, 0, Wakeup::None));
}

#[test]
fn reentering_replaces_the_ghost_and_resumes_from_it() {
    let mut page = page(true, false);
    let (shot, start) = settle(&mut page);
    toggle(&mut page, &shot);
    let mut last = present(&mut page, frame_time(start + 1), true).0;
    for index in start + 2..start + 6 {
        last = present(&mut page, frame_time(index), false).0;
    }
    let midway = ink(&last);
    assert!((1..255).contains(&midway), "exit underway: {midway}");
    toggle(&mut page, &last);
    let mut samples = Vec::new();
    for index in start + 6..start + 40 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == start + 6);
        assert_eq!(panels(&shot), 1, "the entering node replaces its ghost");
        assert!(shot.nodes.iter().any(|n| n.test_id == "panel"));
        samples.push(ink(&shot));
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert!(
        samples[0].abs_diff(midway) <= 40,
        "resumes near the ghost, not from the start: {midway} -> {samples:?}"
    );
    assert_eq!(*samples.last().unwrap(), 0);
    assert!(samples.windows(2).all(|w| w[0] >= w[1]), "{samples:?}");
}

#[test]
fn a_sheet_holding_a_scroll_slides_in_by_composition() {
    let sheet = "h('dialog',{open:shown.value,'aria-label':'Sheet','data-testid':'sheet',className:'starting:down exit:down',style:{marginTop:'auto',marginBottom:0,width:200,height:60,backgroundColor:'#000000',transition:{properties:['transform'],durationMs:200,delayMs:0,easing:'linear'}}},h('scroll',{'data-testid':'inner',style:{height:40}},h('view',{style:{height:300}})))";
    let module = super::source(&format!(
        "function render(){{const shown=useLocal('shown',false);return h('page',{{}},h('button',{{'data-testid':'toggle',onClick(){{shown.set(!shown.value)}},style:{{width:40,height:20}}}},'T'),{sheet})}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'starting:down',when:{state:'starting'},style:{translate:[0,60]}},{name:'exit:down',when:{state:'exit'},style:{translate:[0,60]}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let (shot, _) = present(&mut page, frame_time(0), true);
    toggle(&mut page, &shot);
    let top = |shot: &Shot| {
        shot.nodes
            .iter()
            .find(|n| n.test_id == "inner")
            .map(|n| n.bounds[1])
            .unwrap()
    };
    let mut tops = Vec::new();
    for index in 1..30 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == 1);
        tops.push(top(&shot));
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert_eq!(tops[0], 100.0, "the scroll starts a full sheet height down");
    assert_eq!(*tops.last().unwrap(), 40.0, "and lands at the page bottom");
    assert!(tops.windows(2).all(|w| w[0] >= w[1]), "{tops:?}");
    assert!(
        tops.iter().filter(|t| **t > 40.0 && **t < 100.0).count() >= 8,
        "{tops:?}"
    );
}

#[test]
fn a_sheet_dragged_off_reopens_on_its_resting_point() {
    let sheet = "h('dialog',{open:open.value,onClose(){open.set(false)},closedby:'any','aria-label':'Sheet','data-testid':'sheet',drag:'y',dragSnapPoints:[0,'50%','100%'],bind:at,className:'starting:down exit:down',style:{marginTop:'auto',marginBottom:0,width:200,height:100,backgroundColor:'#000000',transition:{properties:['transform'],durationMs:200,delayMs:0,easing:'linear'}}},h('text',{},'Hi'))";
    let module = super::source(&format!(
        "function render(){{const open=useLocal('open',false);const at=useLocal('at',1);return h('page',{{}},h('button',{{'data-testid':'toggle',onClick(){{at.set(1);open.set(true)}},style:{{width:40,height:20}}}},'T'),{sheet})}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'starting:down',when:{state:'starting'},style:{translate:[0,'100%']}},{name:'exit:down',when:{state:'exit'},style:{translate:[0,'100%']}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let mut frame = 0u64;
    let mut show = |page: &mut ViewSession,
                    offsets: &std::collections::HashMap<String, f32>,
                    rebuild: bool| {
        frame += 1;
        let at = frame_time(frame);
        let nodes = page
            .prepare_view_animated(
                200,
                200,
                1.0,
                rebuild,
                Some(offsets),
                at,
                |display: DisplayList| Ok(display.nodes.clone()),
            )
            .unwrap();
        assert!(
            page.animation_presented(SURFACE, page.generation(), at)
                .unwrap()
        );
        (nodes, page.view_animation_wakeup(at).unwrap())
    };
    let none = std::collections::HashMap::new();
    let (nodes, _) = show(&mut page, &none, true);
    let toggle = nodes
        .iter()
        .find(|n| n.test_id == "toggle")
        .unwrap()
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), toggle).unwrap());
    let mut rebuild = true;
    let mut nodes;
    loop {
        let (next, wakeup) = show(&mut page, &none, rebuild);
        rebuild = false;
        nodes = next;
        if wakeup == Wakeup::None {
            break;
        }
    }
    let panel = nodes.iter().find(|n| n.test_id == "sheet").unwrap().clone();
    assert_eq!(panel.bounds[1], 150.0, "opens on its half point");
    let key = crate::scene::node::DragSnapshot::key(&panel.identity);
    // The finger drags it off and its spring lands on the hiding point.
    let off = std::collections::HashMap::from([(key.clone(), 100.0)]);
    show(&mut page, &off, false);
    let handle = panel.drag.as_ref().unwrap().handle;
    assert!(
        page.text_event(
            page.generation(),
            handle,
            r#"{"phase":"settle","index":2,"velocity":0,"hidden":true}"#
        )
        .unwrap()
    );
    let mut rebuild = true;
    let mut closed = Vec::new();
    for _ in 0..40 {
        let (nodes, wakeup) = show(&mut page, &off, rebuild);
        rebuild = false;
        closed = nodes;
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert!(closed.iter().all(|n| n.test_id != "sheet"));
    let toggle = closed
        .iter()
        .find(|n| n.test_id == "toggle")
        .unwrap()
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), toggle).unwrap());
    let rest = std::collections::HashMap::from([(key, 50.0)]);
    let mut rebuild = true;
    let mut tops = Vec::new();
    for _ in 0..40 {
        let (nodes, wakeup) = show(&mut page, if rebuild { &off } else { &rest }, rebuild);
        rebuild = false;
        tops.push(
            nodes
                .iter()
                .find(|n| n.test_id == "sheet")
                .map(|n| n.bounds[1]),
        );
        if wakeup == Wakeup::None && tops.len() > 1 {
            break;
        }
    }
    assert_eq!(*tops.last().unwrap(), Some(150.0), "{tops:?}");
    assert!(
        tops.iter()
            .any(|t| t.is_some_and(|t| t > 150.0 && t < 250.0)),
        "slides in: {tops:?}"
    );
}
