//! `top_layer_open`, the signal a host lifts the page over its chrome by: a shown dialog in the
//! top layer or its exit ghost, never a popover.
use crate::session::lifecycle::SurfaceTicket;
use crate::{DisplayList, ViewSession, animation::Wakeup};
use std::time::Duration;

const SURFACE: SurfaceTicket = SurfaceTicket::new(1);
const W: usize = 200;

fn frame_time(index: u64) -> Duration {
    Duration::from_micros(index * 16_667)
}

fn environment() -> serde_json::Value {
    serde_json::json!({"appearance":"light","insets":{"top":20,"right":0,"bottom":10,"left":0},"safeArea":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en","canDock":false,"reducedMotion":false})
}

/// A red page with a button inside the top chrome zone, a toggle, and a sheet that slides out.
fn page(overlay: &str) -> ViewSession {
    let overlay = match overlay {
        "dialog" => {
            "h('dialog',{open:shown.value,onClose(){shown.set(false)},closedby:'any','aria-label':'Sheet','data-testid':'sheet',className:'exit:down',style:{marginTop:'auto',marginBottom:0,width:200,height:30,backgroundColor:'#000000',transition:{properties:['transform'],durationMs:200,delayMs:0,easing:'linear'}}})"
        }
        _ => {
            "h('view',{id:'toast',popover:'manual',open:shown.value,'data-testid':'toast',style:{width:50,height:20}})"
        }
    };
    let module = super::source(&format!(
        "function render(){{const shown=useLocal('shown',false);return h('page',{{'data-testid':'root',style:{{backgroundColor:'#ff0000'}}}},h('button',{{'data-testid':'bar',style:{{position:'absolute',left:0,top:0,width:40,height:15}}}},'B'),h('button',{{'data-testid':'toggle',onClick(){{shown.set(!shown.value)}},style:{{position:'absolute',left:150,top:40,width:40,height:20}}}},'T'),{overlay})}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'exit:down',when:{state:'exit'},style:{translate:[0,60]}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_environment(environment()).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    page
}

struct Shot {
    nodes: Vec<crate::scene::node::NodeSnapshot>,
    open: bool,
}

impl Shot {
    fn node(&self, id: &str) -> &crate::scene::node::NodeSnapshot {
        self.nodes.iter().find(|n| n.test_id == id).unwrap()
    }
}

fn present(page: &mut ViewSession, at: Duration, rebuild: bool) -> (Shot, Wakeup) {
    let shot = page
        .prepare_view_animated(
            W as u16,
            100,
            1.0,
            rebuild,
            None,
            at,
            |display: DisplayList| {
                Ok(Shot {
                    nodes: display.nodes.clone(),
                    open: display.top_layer_open(),
                })
            },
        )
        .unwrap();
    assert!(
        page.animation_presented(SURFACE, page.generation(), at)
            .unwrap()
    );
    assert_eq!(page.frame_metrics().top_layer_open, shot.open);
    (shot, page.view_animation_wakeup(at).unwrap())
}

fn click(page: &mut ViewSession, shot: &Shot, id: &str) {
    let handle = shot.node(id).on_click.unwrap();
    assert!(page.click(page.generation(), handle).unwrap());
}

/// Presents from `from` until nothing is scheduled; returns every frame.
fn run(page: &mut ViewSession, from: u64) -> Vec<Shot> {
    let mut shots = Vec::new();
    for index in from..from + 60 {
        let (shot, wakeup) = present(page, frame_time(index), index == from);
        shots.push(shot);
        if wakeup == Wakeup::None {
            return shots;
        }
    }
    panic!("never settled");
}

/// The page after the toggle, settled.
fn opened(overlay: &str) -> (ViewSession, Shot) {
    let mut page = page(overlay);
    let shot = run(&mut page, 0).pop().unwrap();
    click(&mut page, &shot, "toggle");
    let shot = run(&mut page, 100).pop().unwrap();
    (page, shot)
}

#[test]
fn a_shown_dialog_holds_the_top_layer_until_its_exit_ends() {
    let mut page = page("dialog");
    assert!(!run(&mut page, 0).pop().unwrap().open);
    let (mut page, shot) = opened("dialog");
    assert!(shot.open);
    click(&mut page, &shot, "sheet-backdrop");
    let shots = run(&mut page, 200);
    assert!(shots.len() > 4, "the sheet slides out over frames");
    assert!(shots[1].open, "the exit ghost holds the layer");
    assert!(!shots.last().unwrap().open);
}

#[test]
fn a_dialog_without_exit_paint_leaves_at_once_with_its_backdrop_and_schedules_nothing() {
    let module = super::source(
        "function render(){const shown=useLocal('shown',true);return h('page',{},h('dialog',{open:shown.value,onClose(){shown.set(false)},closedby:'any','aria-label':'Sheet','data-testid':'sheet',style:{width:100,height:30}}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_environment(environment()).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let shot = run(&mut page, 0).pop().unwrap();
    assert!(shot.open);
    click(&mut page, &shot, "sheet-backdrop");
    let shots = run(&mut page, 100);
    assert_eq!(shots.len(), 1, "nothing is left to animate");
    assert!(!shots[0].open);
    assert!(shots[0].nodes.iter().all(|n| n.test_id != "sheet-backdrop"));
}

#[test]
fn an_anchored_popover_stays_clear_of_the_bars_over_the_page() {
    let module = super::source(
        "function render(){return h('page',{},h('button',{popoverTarget:'menu','data-testid':'more',style:{position:'absolute',left:10,top:60,width:40,height:10}},'M'),h('view',{id:'menu',popover:'auto','data-testid':'menu',style:{width:60,height:15}}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let mut env = environment();
    // A tab bar 30 high: below the trigger fits the surface but lands under the bar.
    env["insets"]["bottom"] = 30.into();
    page.set_environment(env).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let shot = run(&mut page, 0).pop().unwrap();
    click(&mut page, &shot, "more");
    let shot = run(&mut page, 100).pop().unwrap();
    let [_, y, _, height] = shot.node("menu").bounds;
    assert!(height > 0.0, "the menu shows");
    assert!(
        y + height <= 70.0,
        "the menu clears the tab bar: {y} + {height}"
    );
    assert!(y >= 20.0, "and the top bar");
}

#[test]
fn a_popover_opened_from_a_list_row_sits_against_its_row() {
    let module = super::source(
        "function render(){return h('page',{},h('view',{style:{height:40}}),h('view',{id:'menu',popover:'auto','data-testid':'menu',style:{width:60,height:15}}),h(listTag,{count:3,height:()=>20,style:{height:60},row:i=>h('button',{popoverTarget:'menu','data-testid':'more-'+i,style:{width:40,height:20}},'M')}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_environment(environment()).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let shot = run(&mut page, 0).pop().unwrap();
    click(&mut page, &shot, "more-1");
    let shot = run(&mut page, 100).pop().unwrap();
    let [_, row_y, _, row_height] = shot.node("more-1").bounds;
    let [_, y, _, height] = shot.node("menu").bounds;
    assert!(height > 0.0, "the menu shows");
    assert!(
        (y - (row_y + row_height)).abs() <= 8.0 || (y + height - row_y).abs() <= 8.0,
        "the menu sits against its row, not the surface origin: menu {y}, row {row_y}"
    );
}

#[test]
fn a_popover_placed_by_its_classes_stays_clear_of_the_bars() {
    let module = super::source(
        "function render(){return h('page',{},h('view',{id:'note',popover:'auto',open:true,'data-testid':'note',style:{marginTop:0,marginLeft:0,width:60,height:15}}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let mut env = environment();
    env["insets"]["bottom"] = 30.into();
    page.set_environment(env).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let shot = run(&mut page, 100).pop().unwrap();
    let [x, y, _, height] = shot.node("note").bounds;
    assert!(height > 0.0, "the note shows");
    assert_eq!(x, 0.0, "its classes place it at the start");
    assert!(y >= 20.0, "below the top bar, not under it: {y}");
}

#[test]
fn a_popover_leaves_the_chrome_over_the_page() {
    let (_, shot) = opened("popover");
    assert!(
        shot.node("toast").visible_bounds[2] > 0.0,
        "the toast shows"
    );
    assert!(!shot.open);
}

/// Indices of the chrome images this frame draws, the first page paint, and the first black
/// paint (the backdrop or the sheet), with each chrome image's bounds and whether it is bound.
struct Chrome {
    images: Vec<(usize, [f32; 4], bool)>,
    page: Option<usize>,
    top_layer: Option<usize>,
    shot: Shot,
}

fn present_chrome(page: &mut ViewSession, at: Duration, width: u16) -> (Chrome, u64) {
    use crate::scene::paint::Paint;
    let chrome = page
        .prepare_view_animated(width, 100, 2.0, true, None, at, |display: DisplayList| {
            let color = |paint: &Paint| match paint {
                Paint::Vector { color, .. } | Paint::Rect { color, .. } => Some(*color),
                _ => None,
            };
            Ok(Chrome {
                images: display
                    .commands
                    .iter()
                    .enumerate()
                    .filter_map(|(index, paint)| match paint {
                        Paint::Image { src, bounds, .. }
                            if crate::scene::display::ChromeEdge::of_source(src).is_some() =>
                        {
                            Some((index, *bounds, display.images.contains_key(src)))
                        }
                        _ => None,
                    })
                    .collect(),
                page: display
                    .commands
                    .iter()
                    .position(|p| color(p).is_some_and(|c| c[..3] == [255, 0, 0])),
                top_layer: display
                    .commands
                    .iter()
                    .position(|p| color(p).is_some_and(|c| c[..3] == [0, 0, 0] && c[3] > 0)),
                shot: Shot {
                    nodes: display.nodes.clone(),
                    open: display.top_layer_open(),
                },
            })
        })
        .unwrap();
    page.animation_presented(SURFACE, page.generation(), at)
        .unwrap();
    (chrome, page.frame_metrics().chrome_snapshot)
}

fn snapshot(page: &mut ViewSession, generation: u64) {
    use crate::scene::display::ChromeEdge;
    let rgba = vec![255u8; 4 * 4 * 2];
    page.set_chrome_snapshot(
        ChromeEdge::Top,
        generation,
        &rgba,
        4,
        2,
        [0.0, 0.0, 400.0, 40.0],
    )
    .unwrap();
}

#[test]
fn bar_snapshots_draw_between_the_page_and_its_top_layer() {
    let (mut page, _) = opened("dialog");
    snapshot(&mut page, 7);
    let (chrome, generation) = present_chrome(&mut page, frame_time(300), W as u16 * 2);
    assert_eq!(generation, 7);
    let bound: Vec<_> = chrome.images.iter().filter(|(.., bound)| *bound).collect();
    assert_eq!(bound.len(), 1, "only the top bar has a snapshot");
    let (index, bounds, _) = *bound[0];
    assert_eq!(bounds, [0.0, 0.0, 200.0, 20.0], "surface pixels at scale 2");
    assert!(chrome.page.unwrap() < index, "over the page");
    assert!(
        index < chrome.top_layer.unwrap(),
        "under the backdrop and the dialog"
    );
}

#[test]
fn bar_snapshots_go_when_the_top_layer_closes_or_the_surface_changes() {
    let (mut page, _) = opened("dialog");
    snapshot(&mut page, 1);
    assert_eq!(
        present_chrome(&mut page, frame_time(300), W as u16 * 2).1,
        1
    );
    // A resize: the host captures the bars again.
    assert_eq!(
        present_chrome(&mut page, frame_time(301), W as u16 * 2 + 2).1,
        0
    );
    snapshot(&mut page, 2);
    let (chrome, generation) = present_chrome(&mut page, frame_time(302), W as u16 * 2 + 2);
    assert_eq!(generation, 2);
    click(&mut page, &chrome.shot, "sheet-backdrop");
    let mut index = 400;
    loop {
        let (_, generation) = present_chrome(&mut page, frame_time(index), W as u16 * 2 + 2);
        if generation == 0 {
            break;
        }
        index += 1;
        assert!(index < 460, "the exit ends");
    }
    assert!(index > 400, "the exit ghost keeps the bars dimmed");
    // Reopened: nothing to draw until the host captures again.
    let shot = run(&mut page, 500).pop().unwrap();
    click(&mut page, &shot, "toggle");
    let (chrome, generation) = present_chrome(&mut page, frame_time(600), W as u16 * 2 + 2);
    assert_eq!(generation, 0);
    assert!(chrome.images.iter().all(|(.., bound)| !bound));
}

#[test]
fn a_page_without_a_top_layer_has_no_bar_slots() {
    let mut page = page("popover");
    run(&mut page, 0);
    snapshot(&mut page, 3);
    let (chrome, generation) = present_chrome(&mut page, frame_time(100), W as u16 * 2);
    assert!(chrome.images.is_empty());
    assert_eq!(generation, 0);
}

/// Where `push_under_top_layer` puts a marker paint this frame, against the first page paint,
/// the first chrome slot and the last command; with whether the animated spans stay ordered.
fn under_top_layer(
    page: &mut ViewSession,
    at: Duration,
) -> (usize, Option<usize>, Option<usize>, usize, bool) {
    use crate::scene::paint::Paint;
    let rect = |color| Paint::Rect {
        color,
        bounds: [0.0, 90.0, 200.0, 10.0],
        radius: 0.0,
        corners: None,
    };
    page.prepare_view_animated(W as u16 * 2, 100, 2.0, true, None, at, |mut display: DisplayList| {
        display.push_under_top_layer("material", vec![rect([1, 2, 3, 255])]);
        let marker = display
            .commands
            .iter()
            .position(|p| matches!(p, Paint::Rect { color: [1, 2, 3, 255], .. }))
            .unwrap();
        let page = display
            .commands
            .iter()
            .position(|p| matches!(p, Paint::Rect { color, .. } if color[..3] == [255, 0, 0]));
        let slot = display.commands.iter().position(|p| {
            matches!(p, Paint::Image { src, .. } if crate::scene::display::ChromeEdge::of_source(src).is_some())
        });
        let spans = display.animated_spans();
        let ordered = spans.windows(2).all(|pair| pair[0].commands.end <= pair[1].commands.start)
            && spans.iter().any(|span| span.identity == "material" && span.commands == (marker..marker + 1));
        Ok((marker, page, slot, display.commands.len(), ordered))
    })
    .unwrap()
}

#[test]
fn the_bar_material_sits_under_an_open_dialog() {
    let (mut page, _) = opened("dialog");
    let (marker, page_paint, slot, _, ordered) = under_top_layer(&mut page, frame_time(300));
    assert!(page_paint.unwrap() < marker, "over the page");
    assert_eq!(
        slot,
        Some(marker + 1),
        "under the bar snapshots and the dialog"
    );
    assert!(ordered, "its own span, in order");
}

#[test]
fn without_a_dialog_the_bar_material_is_drawn_last() {
    let (mut page, _) = opened("popover");
    let (marker, _, slot, len, ordered) = under_top_layer(&mut page, frame_time(300));
    assert_eq!(marker, len - 1, "over the page and a popover");
    assert!(slot.is_none() || slot < Some(marker));
    assert!(ordered);
}
