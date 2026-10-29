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
            "h('scroll',{{'data-testid':'s',style:{{height:30,width:100}}}},h('column',{{style:{{height:300,paddingTop:20}}}},shown.value && {panel}))"
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
    page.set_environment(serde_json::json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"runner":false,"reducedMotion":true})).unwrap();
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
