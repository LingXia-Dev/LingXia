//! `transition-size` on a fake 60 Hz clock: a resized box paints from the size it had to the
//! size layout gives it, clipping its children at the size reached.
use crate::session::lifecycle::SurfaceTicket;
use crate::{DisplayList, ViewSession, animation::Wakeup};
use std::time::Duration;

const SURFACE: SurfaceTicket = SurfaceTicket::new(1);

fn frame_time(index: u64) -> Duration {
    Duration::from_micros(index * 16_667)
}

/// A red panel 100 wide, 20 high closed and 80 open, holding a blue block 50 wide and always
/// 80 high; a green row follows it, with `extra` in its style.
fn page(extra: &str) -> ViewSession {
    let module = super::source(&format!(
        "function render(){{const open=useLocal('open',false);const motion=p=>({{properties:[p],durationMs:200,delayMs:0,easing:'linear'}});return h('page',{{style:{{backgroundColor:'#ffffff'}}}},h('button',{{'data-testid':'toggle',onClick(){{open.set(!open.value)}},style:{{position:'absolute',left:150,top:0,width:40,height:10}}}},'T'),h('view',{{'data-testid':'panel',style:{{width:100,height:open.value?80:20,overflow:'hidden',backgroundColor:'#ff0000',transition:motion('size')}}}},h('view',{{style:{{width:50,height:80,minHeight:80,backgroundColor:'#0000ff'}}}})),h('view',{{'data-testid':'after',style:{{width:100,height:10,backgroundColor:'#00ff00'{extra}}}}}))}}"
    ));
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

/// How many rows of column `x` are mostly `channel` (0 red, 1 green, 2 blue).
fn rows(shot: &Shot, x: usize, channel: usize) -> usize {
    (0..100)
        .filter(|y| {
            let at = (y * 200 + x) * 4;
            (0..3).all(|c| {
                if c == channel {
                    shot.pixels[at + c] > 200
                } else {
                    shot.pixels[at + c] < 60
                }
            })
        })
        .count()
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

fn height(shot: &Shot, id: &str) -> f32 {
    shot.nodes.iter().find(|n| n.test_id == id).unwrap().bounds[3]
}

/// Each frame's painted panel height (red at x = 75) and shown child height (blue at x = 25).
fn run(page: &mut ViewSession, from: u64) -> (Vec<(usize, usize)>, Shot) {
    let mut frames = Vec::new();
    for index in from..from + 40 {
        let (shot, wakeup) = present(page, frame_time(index), index == from);
        frames.push((rows(&shot, 75, 0), rows(&shot, 25, 2)));
        if wakeup == Wakeup::None {
            return (frames, shot);
        }
    }
    panic!("the transition never ends: {frames:?}");
}

#[test]
fn a_box_grows_and_shrinks_through_the_sizes_between_and_clips_its_children_there() {
    let mut page = page("");
    let (shot, _) = present(&mut page, frame_time(0), true);
    assert_eq!((rows(&shot, 75, 0), rows(&shot, 25, 2)), (20, 20));
    toggle(&mut page, &shot);
    let (frames, shot) = run(&mut page, 1);
    let opening = frames.len() as u64;
    assert_eq!(frames[0], (20, 20), "the first frame shows the old size");
    assert_eq!(*frames.last().unwrap(), (80, 80));
    assert_eq!(height(&shot, "panel"), 80.0);
    assert!(frames.windows(2).all(|w| w[0].0 <= w[1].0), "{frames:?}");
    assert!(
        frames.iter().filter(|f| f.0 > 22 && f.0 < 78).count() >= 8,
        "{frames:?}"
    );
    assert!(
        frames.iter().all(|(panel, child)| panel == child),
        "the child shows as far as the panel has got: {frames:?}"
    );
    assert_eq!(page.frame_metrics().layout_motions, 0);
    // A frame after the open one has settled, as a later tap would find the page.
    let (shot, _) = present(&mut page, frame_time(1 + opening), false);
    toggle(&mut page, &shot);
    let (frames, shot) = run(&mut page, 2 + opening);
    // A transition that follows a finished one starts up to two frames in, whatever it animates.
    assert!(
        frames[0].0 >= 68 && frames[0].0 == frames[0].1,
        "it closes from the open size: {frames:?}"
    );
    assert_eq!(*frames.last().unwrap(), (20, 20));
    assert_eq!(height(&shot, "panel"), 20.0);
    assert!(frames.windows(2).all(|w| w[0].0 >= w[1].0), "{frames:?}");
    assert!(
        frames.iter().filter(|f| f.0 > 22 && f.0 < 78).count() >= 8,
        "{frames:?}"
    );
    assert!(
        frames.iter().all(|(panel, child)| panel == child),
        "{frames:?}"
    );
}

#[test]
fn layout_takes_the_new_size_at_once_and_a_follower_with_layout_motion_keeps_to_the_edge() {
    let mut page =
        page(",transition:{properties:['layout'],durationMs:200,delayMs:0,easing:'linear'}");
    let (mut shot, _) = present(&mut page, frame_time(0), true);
    let mut index = 0;
    for (laid_out, lands) in [(80.0, 80), (20.0, 20)] {
        toggle(&mut page, &shot);
        let first = index + 1;
        loop {
            index += 1;
            assert!(index < first + 40, "the transition never ends");
            let (next, wakeup) = present(&mut page, frame_time(index), index == first);
            shot = next;
            assert_eq!(height(&shot, "panel"), laid_out, "layout does not animate");
            let panel = rows(&shot, 75, 0);
            // The green row sits right under the red panel's painted edge on every frame.
            let green = (0..100).find(|y| {
                let at = (y * 200 + 75) * 4;
                shot.pixels[at + 1] > 200 && shot.pixels[at] < 60 && shot.pixels[at + 2] < 60
            });
            assert!(
                green.is_some_and(|top| top.abs_diff(panel) <= 1),
                "frame {index}: panel {panel}, follower {green:?}"
            );
            if wakeup == Wakeup::None {
                assert_eq!(panel, lands);
                break;
            }
        }
    }
}

#[test]
fn reversing_mid_flight_continues_from_the_size_on_screen() {
    let mut page = page("");
    let (shot, _) = present(&mut page, frame_time(0), true);
    toggle(&mut page, &shot);
    let mut last = 20;
    let mut shot = shot;
    for index in 1..7 {
        shot = present(&mut page, frame_time(index), index == 1).0;
        last = rows(&shot, 75, 0);
    }
    assert!(last > 30 && last < 70, "{last}");
    toggle(&mut page, &shot);
    let (frames, _) = run(&mut page, 7);
    assert!(
        frames[0].0.abs_diff(last) <= 6,
        "no jump: {last} then {frames:?}"
    );
    assert!(frames.windows(2).all(|w| w[0].0 >= w[1].0), "{frames:?}");
    assert_eq!(frames.last().unwrap().0, 20);
}
