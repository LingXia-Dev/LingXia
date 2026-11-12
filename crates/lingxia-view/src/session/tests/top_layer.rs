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
    serde_json::json!({"theme":"light","insets":{"top":20,"right":0,"bottom":10,"left":0},"safeArea":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"reducedMotion":false})
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
fn a_popover_leaves_the_chrome_over_the_page() {
    let (_, shot) = opened("popover");
    assert!(
        shot.node("toast").visible_bounds[2] > 0.0,
        "the toast shows"
    );
    assert!(!shot.open);
}
