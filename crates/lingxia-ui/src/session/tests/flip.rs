//! Layout motion (FLIP) on a fake 60 Hz clock: `transition-layout` slides moved boxes into place.
use crate::session::lifecycle::SurfaceTicket;
use crate::{DisplayList, ViewSession, animation::Wakeup};
use std::time::Duration;

const SURFACE: SurfaceTicket = SurfaceTicket::new(1);

fn frame_time(index: u64) -> Duration {
    Duration::from_micros(index * 16_667)
}

/// Three keyed 20-high rows, red, green and blue; the toggle reverses their order.
/// Red is wider, so the column at x = 55 shows it even where the rows cross.
fn page() -> ViewSession {
    let module = super::source(
        "function render(){const flipped=useLocal('flipped',false);const rows=[['a','#ff0000'],['b','#00ff00'],['c','#0000ff']];const order=flipped.value?[...rows].reverse():rows;return h('page',{style:{backgroundColor:'#ffffff'}},h('button',{testId:'toggle',onClick(){flipped.set(!flipped.value)},style:{position:'absolute',left:150,top:0,width:40,height:20}},'T'),h('column',{style:{width:70,alignItems:'flex-start'}},order.map(([key,color])=>h('view',{key,testId:key,style:{height:20,width:key==='a'?60:50,backgroundColor:color,transition:{properties:['layout'],durationMs:200,delayMs:0,easing:'linear'}}}))))}",
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
    assert!(
        page.animation_presented(SURFACE, page.generation(), at)
            .unwrap()
    );
    (shot, page.view_animation_wakeup(at).unwrap())
}

/// The mean row of the red box at x = 55, in logical pixels from the top.
fn red_center(shot: &Shot) -> f64 {
    let rows: Vec<usize> = (0..100)
        .filter(|y| {
            let at = (y * 200 + 55) * 4;
            shot.pixels[at] > 200 && shot.pixels[at + 1] < 60 && shot.pixels[at + 2] < 60
        })
        .collect();
    assert!(!rows.is_empty(), "the red row is drawn");
    rows.iter().sum::<usize>() as f64 / rows.len() as f64 + 0.5
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
fn a_reorder_slides_each_moved_row_from_where_it_was() {
    let mut page = page();
    let (shot, _) = present(&mut page, frame_time(0), true);
    assert_eq!(red_center(&shot), 10.0);
    toggle(&mut page, &shot);
    let mut centers = Vec::new();
    for index in 1..30 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == 1);
        if index == 2 {
            assert_eq!(
                page.frame_metrics().layout_motions,
                2,
                "the middle row stays"
            );
        }
        centers.push(red_center(&shot));
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert_eq!(centers[0], 10.0, "the first frame shows the old position");
    assert_eq!(
        *centers.last().unwrap(),
        50.0,
        "it lands where layout put it"
    );
    assert!(centers.windows(2).all(|w| w[0] <= w[1]), "{centers:?}");
    assert!(
        centers.iter().filter(|c| **c > 11.0 && **c < 49.0).count() >= 8,
        "{centers:?}"
    );
    assert_eq!(page.frame_metrics().layout_motions, 0);
}

#[test]
fn reversing_mid_flight_continues_from_the_position_on_screen() {
    let mut page = page();
    let (shot, _) = present(&mut page, frame_time(0), true);
    toggle(&mut page, &shot);
    let mut last = present(&mut page, frame_time(1), true).0;
    for index in 2..7 {
        last = present(&mut page, frame_time(index), false).0;
    }
    let midway = red_center(&last);
    assert!(midway > 11.0 && midway < 49.0, "{midway}");
    toggle(&mut page, &last);
    let mut centers = Vec::new();
    for index in 7..40 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == 7);
        centers.push(red_center(&shot));
        if wakeup == Wakeup::None {
            break;
        }
    }
    // One frame of the first motion (40 px over 12 frames) separates the samples.
    assert!(
        (centers[0] - midway).abs() <= 4.0,
        "no jump: {midway} -> {centers:?}"
    );
    assert_eq!(*centers.last().unwrap(), 10.0);
    assert!(centers.windows(2).all(|w| w[0] >= w[1]), "{centers:?}");
}

#[test]
fn reduced_motion_lands_a_reorder_at_once() {
    let mut page = page();
    page.set_environment(serde_json::json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"runner":false,"reducedMotion":true})).unwrap();
    let (shot, _) = present(&mut page, frame_time(0), true);
    toggle(&mut page, &shot);
    let (shot, wakeup) = present(&mut page, frame_time(1), true);
    assert_eq!((red_center(&shot), wakeup), (50.0, Wakeup::None));
}

#[test]
fn a_keyed_list_reorder_slides_its_rows() {
    let module = super::source(
        "function render(){const flipped=useLocal('flipped',false);const keys=flipped.value?['c','b','a']:['a','b','c'];const colors={a:'#ff0000',b:'#00ff00',c:'#0000ff'};return h('page',{style:{backgroundColor:'#ffffff'}},h('button',{testId:'toggle',onClick(){flipped.set(!flipped.value)},style:{position:'absolute',left:150,top:0,width:40,height:20}},'T'),h('list',{style:{height:80,width:70},count:3,itemKeys:keys,height:()=>20,row:i=>h('view',{key:keys[i],style:{height:20,width:keys[i]==='a'?60:50,backgroundColor:colors[keys[i]],transition:{properties:['layout'],durationMs:200,delayMs:0,easing:'linear'}}})}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let (shot, _) = present(&mut page, frame_time(0), true);
    assert_eq!(red_center(&shot), 10.0);
    toggle(&mut page, &shot);
    let mut centers = Vec::new();
    for index in 1..30 {
        let (shot, wakeup) = present(&mut page, frame_time(index), index == 1);
        centers.push(red_center(&shot));
        if wakeup == Wakeup::None {
            break;
        }
    }
    assert_eq!((centers[0], *centers.last().unwrap()), (10.0, 50.0));
    assert!(
        centers.iter().filter(|c| **c > 11.0 && **c < 49.0).count() >= 8,
        "{centers:?}"
    );
}
