//! Transitions on a fake 60 Hz clock: presented values, interruption, hit geometry, motion.
use crate::session::lifecycle::SurfaceTicket;
use crate::{DisplayList, ViewSession, animation::Wakeup};
use serde_json::json;
use std::time::Duration;

const SURFACE: SurfaceTicket = SurfaceTicket::new(1);

fn frame_time(index: u64) -> Duration {
    Duration::from_micros(index * 16_667)
}

/// A 100x40 button at the origin whose `active:` delta is `pressed`.
fn page(transition: serde_json::Value, base: &str, pressed: &str, child: &str) -> ViewSession {
    let module = super::source(&format!(
        "function render(){{return h('page',{{style:{{backgroundColor:'#ffffff'}}}},h('button',{{'data-testid':'b',onClick(){{}},className:'active:x',style:{{position:'absolute',left:0,top:0,width:100,height:40,padding:0,borderRadius:0,{base},transition:{transition}}}}},{child}))}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        &format!(
            "configureClasses([{{name:'active:x',when:{{state:'active'}},style:{{{pressed}}}}}]);__lingxiaPublish(render);"
        ),
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    page
}

fn transition(properties: &[&str], ms: u64) -> serde_json::Value {
    json!({"properties":properties,"durationMs":ms,"delayMs":0,"easing":"linear"})
}

/// A presented frame: its nodes, first text colour and CPU pixels.
struct Shot {
    nodes: Vec<crate::scene::node::NodeSnapshot>,
    text: Option<[u8; 4]>,
    pixels: Vec<u8>,
}

fn shoot(display: DisplayList, composited: bool) -> Shot {
    let text = display.commands.iter().find_map(|paint| match paint {
        crate::scene::paint::Paint::Text { color, .. } => Some(*color),
        _ => None,
    });
    let nodes = display.nodes.clone();
    let frame = if composited {
        display.rasterize_composited()
    } else {
        display.rasterize()
    }
    .unwrap();
    Shot {
        nodes,
        text,
        pixels: frame.pixels.data_as_u8_slice().to_vec(),
    }
}

/// Prepare and present one frame at `at`; returns it and the next wakeup.
fn present(page: &mut ViewSession, at: Duration) -> (Shot, Wakeup) {
    present_route(page, at, false, false)
}

fn present_route(
    page: &mut ViewSession,
    at: Duration,
    rebuild: bool,
    composited: bool,
) -> (Shot, Wakeup) {
    let display = page
        .prepare_view_animated(200, 100, 1.0, rebuild, None, at, |display| {
            Ok(shoot(display, composited))
        })
        .unwrap();
    assert!(
        page.animation_presented(SURFACE, page.generation(), at)
            .unwrap()
    );
    (display, page.view_animation_wakeup(at).unwrap())
}

fn press(page: &mut ViewSession, display: &Shot, pressed: bool) {
    let identity = display
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .identity
        .clone();
    page.set_pointer_state(pressed.then_some(&*identity), None);
}

fn pixel(display: &Shot, x: usize, y: usize) -> [u8; 4] {
    display.pixels[(y * 200 + x) * 4..(y * 200 + x) * 4 + 4]
        .try_into()
        .unwrap()
}

fn button(display: &Shot) -> &crate::scene::node::NodeSnapshot {
    display.nodes.iter().find(|n| n.test_id == "b").unwrap()
}

#[test]
fn a_200ms_opacity_transition_takes_200ms_with_intermediate_frames() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    let (display, wakeup) = present(&mut page, frame_time(0));
    assert_eq!(wakeup, Wakeup::None);
    press(&mut page, &display, true);
    let mut samples = Vec::new();
    let mut settled = None;
    for index in 1..40 {
        let (display, wakeup) = present(&mut page, frame_time(index));
        samples.push(pixel(&display, 50, 20)[0]);
        if wakeup == Wakeup::None {
            settled = Some(index);
            break;
        }
        assert_eq!(wakeup, Wakeup::NextFrame);
    }
    // Started on presentation at frame 1; 200 ms is 12 frames at 60 Hz.
    let settled = settled.unwrap();
    assert!((12..=14).contains(&(settled - 1)), "settled at {settled}");
    assert_eq!(*samples.last().unwrap(), 255);
    assert_eq!(samples[0], 0, "the first frame shows the presented value");
    let intermediate = samples.iter().filter(|v| (1..255).contains(*v)).count();
    assert!(intermediate >= 10, "{samples:?}");
    assert!(samples.windows(2).all(|w| w[0] <= w[1]), "{samples:?}");
    // Nothing further is scheduled and later frames stay at the end value.
    let (display, wakeup) = present(&mut page, frame_time(60));
    assert_eq!((pixel(&display, 50, 20)[0], wakeup), (255, Wakeup::None));
}

#[test]
fn interruption_continues_from_the_presented_value_without_a_jump() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    press(&mut page, &display, true);
    let mut last = 0;
    for index in 1..=7 {
        last = pixel(&present(&mut page, frame_time(index)).0, 50, 20)[0];
    }
    assert!((100..160).contains(&last), "{last}");
    press(&mut page, &display, false);
    let mut values = vec![last];
    for index in 8..30 {
        let (display, wakeup) = present(&mut page, frame_time(index));
        values.push(pixel(&display, 50, 20)[0]);
        if wakeup == Wakeup::None {
            break;
        }
    }
    // The reversal's first frame samples the old curve at its own time (one frame step on),
    // then darkens monotonically back to opaque black.
    assert!(values[1].abs_diff(last) <= 22, "{values:?}");
    let values = &values[1..];
    assert!(values.windows(2).all(|w| w[0] >= w[1]), "{values:?}");
    assert_eq!(*values.last().unwrap(), 0);
}

#[test]
fn colours_mix_in_oklab_and_reach_inheriting_text() {
    let mut page = page(
        transition(&["bg", "fg"], 200),
        "backgroundColor:'#000000',color:'#ffffff'",
        "backgroundColor:'#ffffff',color:'#000000'",
        "h('text',{'data-testid':'t'},'Hello')",
    );
    let (display, _) = present(&mut page, frame_time(0));
    let text_color = |display: &Shot| display.text.unwrap();
    assert_eq!(text_color(&display), [255, 255, 255, 255]);
    press(&mut page, &display, true);
    present(&mut page, Duration::from_millis(100));
    let (display, _) = present(&mut page, Duration::from_millis(200));
    // Linear easing at the midpoint: OKLab middle gray, not sRGB 128.
    assert_eq!(pixel(&display, 2, 2), [99, 99, 99, 255]);
    assert_eq!(text_color(&display), [99, 99, 99, 255]);
    let (display, wakeup) = present(&mut page, Duration::from_millis(300));
    assert_eq!(pixel(&display, 2, 2), [255, 255, 255, 255]);
    assert_eq!(text_color(&display), [0, 0, 0, 255]);
    assert_eq!(wakeup, Wakeup::None);
}

#[test]
fn a_background_fades_in_from_transparent_and_shadows_interpolate_per_layer() {
    let mut page = page(
        transition(&["bg", "shadow"], 200),
        "backgroundColor:'#00000000',boxShadow:[{offsetX:0,offsetY:0,blur:0,spread:0,color:'#00000000'}]",
        "backgroundColor:'#000000',boxShadow:[{offsetX:0,offsetY:20,blur:0,spread:0,color:'#000000'}]",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    assert_eq!(pixel(&display, 50, 20), [255, 255, 255, 255]);
    press(&mut page, &display, true);
    present(&mut page, Duration::from_millis(100));
    let (display, _) = present(&mut page, Duration::from_millis(200));
    let fill = pixel(&display, 50, 20);
    assert!(
        (100..160).contains(&fill[0]),
        "half-opaque black over white: {fill:?}"
    );
    // The shadow has moved 10 px down: covered at y=45, not yet at y=55.
    assert!(pixel(&display, 50, 45)[0] < 250);
    assert_eq!(pixel(&display, 50, 55)[0], 255);
    let (display, _) = present(&mut page, Duration::from_millis(300));
    assert_eq!(pixel(&display, 50, 20), [0, 0, 0, 255]);
    assert_eq!(pixel(&display, 50, 55)[0], 0);
}

#[test]
fn hit_geometry_follows_the_presented_transform_mid_flight() {
    let mut page = page(
        transition(&["transform"], 200),
        "backgroundColor:'#000000'",
        "scale:[0.5,0.5]",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    assert_eq!(button(&display).bounds, [0.0, 0.0, 100.0, 40.0]);
    press(&mut page, &display, true);
    present(&mut page, Duration::from_millis(100));
    let (display, _) = present(&mut page, Duration::from_millis(200));
    let bounds = button(&display).bounds;
    // Scale 0.75 about the center.
    for (actual, expected) in bounds.iter().zip([12.5, 5.0, 75.0, 30.0]) {
        assert!((actual - expected).abs() < 0.01, "{bounds:?}");
    }
    assert!(button(&display).on_click.is_some());
    assert_eq!(pixel(&display, 5, 2)[0], 255);
    assert_eq!(pixel(&display, 20, 10)[0], 0);
    let (display, _) = present(&mut page, Duration::from_millis(300));
    assert_eq!(button(&display).bounds, [25.0, 10.0, 50.0, 20.0]);
}

#[test]
fn translation_transitions_replace_the_laid_out_offset() {
    let mut page = page(
        transition(&["transform"], 200),
        "backgroundColor:'#000000'",
        "translate:[0,20]",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    press(&mut page, &display, true);
    present(&mut page, Duration::from_millis(100));
    let (display, _) = present(&mut page, Duration::from_millis(200));
    assert!((button(&display).bounds[1] - 10.0).abs() < 0.01);
    let (display, _) = present(&mut page, Duration::from_millis(300));
    assert_eq!(button(&display).bounds[1], 20.0);
}

#[test]
fn cpu_and_composited_routes_sample_the_same_values_at_fixed_times() {
    let pages = || {
        page(
            transition(&["opacity", "transform", "bg"], 200),
            "backgroundColor:'#ff0000',rotate:0",
            "opacity:0.2,rotate:45,backgroundColor:'#0000ff'",
            "h('text',{},'Go')",
        )
    };
    let (mut cpu, mut composited) = (pages(), pages());
    for page in [&mut cpu, &mut composited] {
        let (display, _) = present(page, frame_time(0));
        press(page, &display, true);
        present(page, Duration::from_millis(20));
    }
    for at in [60, 120, 180] {
        let at = Duration::from_millis(20 + at);
        let (a, _) = present_route(&mut cpu, at, false, false);
        let (b, _) = present_route(&mut composited, at, false, true);
        assert_ne!(
            a.pixels,
            present_route(&mut pages(), at, true, false).0.pixels
        );
        for (index, (a, b)) in a.pixels.iter().zip(&b.pixels).enumerate() {
            assert!(a.abs_diff(*b) <= 2, "t={at:?} byte={index}: {a} vs {b}");
        }
    }
}

#[test]
fn reduced_motion_lands_on_the_final_state_in_one_frame() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    let mut environment = json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"reducedMotion":true});
    page.set_environment(environment.clone()).unwrap();
    let (display, _) = page_frame(&mut page, 0);
    press(&mut page, &display, true);
    let (display, wakeup) = page_frame(&mut page, 1);
    assert_eq!((pixel(&display, 50, 20)[0], wakeup), (255, Wakeup::None));
    // Turning it off again animates the next change.
    environment["reducedMotion"] = json!(false);
    page.set_environment(environment).unwrap();
    page_frame(&mut page, 2);
    press(&mut page, &display, false);
    let (_, wakeup) = page_frame(&mut page, 3);
    assert_eq!(wakeup, Wakeup::NextFrame);
}

fn page_frame(page: &mut ViewSession, index: u64) -> (Shot, Wakeup) {
    present_route(page, frame_time(index), true, false)
}

#[test]
fn hidden_pages_park_running_transitions_and_jump_new_ones_to_their_end() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    press(&mut page, &display, true);
    for index in 1..=7 {
        present(&mut page, frame_time(index));
    }
    let running = pixel(&present(&mut page, frame_time(7)).0, 50, 20)[0];
    // Hide: a catch-up frame samples the parked time and schedules nothing.
    page.set_animation_target(Some(SURFACE), false, frame_time(7))
        .unwrap();
    let (parked, wakeup) = present(&mut page, Duration::from_secs(5));
    assert_eq!((pixel(&parked, 50, 20)[0], wakeup), (running, Wakeup::None));
    // Reveal: time resumes from the parked value.
    page.set_animation_target(Some(SURFACE), true, Duration::from_secs(6))
        .unwrap();
    let (shown, wakeup) = present(&mut page, Duration::from_secs(6));
    assert_eq!(
        (pixel(&shown, 50, 20)[0], wakeup),
        (running, Wakeup::NextFrame)
    );
    // A change while hidden lands on its end value.
    page.set_animation_target(Some(SURFACE), false, Duration::from_secs(6))
        .unwrap();
    press(&mut page, &display, false);
    let (hidden, wakeup) = present(&mut page, Duration::from_secs(7));
    assert_eq!((pixel(&hidden, 50, 20)[0], wakeup), (0, Wakeup::None));
    page.set_animation_target(Some(SURFACE), true, Duration::from_secs(8))
        .unwrap();
    let (shown, wakeup) = present(&mut page, Duration::from_secs(8));
    assert_eq!((pixel(&shown, 50, 20)[0], wakeup), (0, Wakeup::None));
}

fn environment(reduced: bool, scale: f64) -> serde_json::Value {
    json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"reducedMotion":reduced,"animatorDurationScale":scale})
}

#[test]
fn the_animator_duration_scale_stretches_transitions() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    page.set_environment(environment(false, 2.0)).unwrap();
    let (display, _) = page_frame(&mut page, 0);
    press(&mut page, &display, true);
    present(&mut page, Duration::from_millis(100));
    let (half, _) = present(&mut page, Duration::from_millis(300));
    assert!((120..136).contains(&pixel(&half, 50, 20)[0]));
    let (_, wakeup) = present(&mut page, Duration::from_millis(499));
    assert_eq!(wakeup, Wakeup::NextFrame);
    let (done, wakeup) = present(&mut page, Duration::from_millis(501));
    assert_eq!((pixel(&done, 50, 20)[0], wakeup), (255, Wakeup::None));
}

#[test]
fn reduced_motion_keeps_only_animations_authored_for_it() {
    let pulse = |when: &str| {
        json!({"name":"pulse","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":when,"easing":"linear",
            "opacity":[{"offset":0,"value":1},{"offset":0.5,"value":0},{"offset":1,"value":1}],"transforms":[]})
    };
    for (when, plays) in [("base", false), ("keep", true)] {
        let module = super::source(&format!(
            "function render(){{return h('page',{{style:{{backgroundColor:'#ffffff'}}}},h('column',{{style:{{width:40,height:40,backgroundColor:'#000000',animation:{}}}}}))}}",
            pulse(when)
        ));
        let mut page = ViewSession::new(&module).unwrap();
        page.set_animation_surface(Some(SURFACE), Duration::ZERO)
            .unwrap();
        page.set_environment(environment(false, 0.0)).unwrap();
        present_route(&mut page, Duration::ZERO, true, false);
        let (frame, wakeup) = present_route(&mut page, Duration::from_millis(500), false, false);
        assert_eq!(pixel(&frame, 20, 20)[0] == 255, plays, "{when}");
        assert_eq!(wakeup == Wakeup::NextFrame, plays, "{when}");
    }
}

/// Every span still covers the spinner's own paints after the frame's splices.
fn assert_spans_aligned(display: &DisplayList) {
    use crate::scene::paint::Paint;
    let spans = display.animated_spans();
    assert_eq!(spans.len(), 1);
    let span = &spans[0];
    let commands = &display.commands[span.commands.clone()];
    assert!(
        commands.iter().any(|p| matches!(
            p,
            Paint::Rect {
                color: [0, 255, 0, 255],
                ..
            }
        )),
        "the span holds the spinner's fill"
    );
    assert!(
        !commands.iter().any(|p| matches!(p, Paint::Text { .. })),
        "no text, caret or selection paint inside the spinner's span"
    );
    let mut depth = 0i32;
    for paint in commands {
        match paint {
            Paint::Clip(_) | Paint::Opacity { .. } | Paint::Backdrop { .. } => depth += 1,
            Paint::PopClip | Paint::PopOpacity | Paint::PopBackdrop => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0);
    }
    assert_eq!(depth, 0);
    assert!(matches!(
        display.commands[span.commands.end],
        Paint::Transform(_)
    ));
}

#[test]
fn spans_stay_on_their_commands_through_caret_input_and_selection_splices() {
    use serde_json::Value;
    let spin = r#"{"name":"spin","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":"linear","opacity":null,"transforms":[{"kind":"rotate","angle":[{"offset":0,"value":0},{"offset":1,"value":360}]}]}"#;
    let module = super::source(&format!(
        "function render(){{const draft=useLocal('draft','typed');return h('column',{{style:{{backgroundColor:'#ffffff'}}}},h('input',{{'data-testid':'i',bind:draft,style:{{width:150}}}}),h('column',{{style:{{width:20,height:20,backgroundColor:'#00ff00',animation:{spin}}}}}),h('text',{{'data-testid':'p',style:{{userSelect:'text',fontSize:16,width:180}}}},'hello brave new world'))}}"
    ));
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let frame = |page: &mut ViewSession, at: u64, caret: bool| {
        let at = frame_time(at);
        let checked = page
            .prepare_view_animated(200, 160, 1.0, true, None, at, |mut display| {
                assert_spans_aligned(&display);
                if !caret {
                    display.hide_caret();
                    assert_spans_aligned(&display);
                }
                Ok(display.nodes.clone())
            })
            .unwrap();
        page.animation_presented(SURFACE, page.generation(), at)
            .unwrap();
        checked
    };
    let nodes = frame(&mut page, 0, true);
    let input = nodes
        .iter()
        .find(|n| n.test_id == "i")
        .unwrap()
        .identity
        .clone();
    let reply: Value = serde_json::from_str(
        &page
            .input_request(
                &json!({"op":"focus","identity":input,"generation":page.generation()}).to_string(),
            )
            .unwrap(),
    )
    .unwrap();
    assert!(reply["token"].as_u64().is_some());
    // Focused: the input decorates (caret, selection) before the span.
    frame(&mut page, 1, true);
    frame(&mut page, 2, false);
    let nodes = frame(&mut page, 3, true);
    let text = nodes.iter().find(|n| n.test_id == "p").unwrap();
    let point = [text.bounds[0] + 60.0, text.bounds[1] + text.bounds[3] / 2.0];
    assert!(
        page.select_text(page.generation(), &text.identity.clone(), point)
            .unwrap()
    );
    // A static-text selection decorates after the span.
    frame(&mut page, 4, true);
    frame(&mut page, 5, false);
}

#[test]
fn a_transition_continues_evenly_through_frames_prepared_ahead() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    press(&mut page, &display, true);
    let sampled = |page: &mut ViewSession, now: u64, sample: u64, ahead: bool| {
        let shot = page
            .prepare_view_sampled(
                200,
                100,
                1.0,
                false,
                None,
                frame_time(now),
                frame_time(sample),
                ahead,
                |display| Ok(shoot(display, false)),
            )
            .unwrap();
        (pixel(&shot, 50, 20)[0], page.generation())
    };
    // N samples the vsync after next; N+1 is prepared before N presents and samples the
    // one after that; both acknowledge in order.
    let (n, gn) = sampled(&mut page, 1, 2, false);
    let (n1, gn1) = sampled(&mut page, 1, 3, true);
    assert!(
        page.animation_presented(SURFACE, gn, frame_time(2))
            .unwrap()
    );
    assert!(
        page.animation_presented(SURFACE, gn1, frame_time(3))
            .unwrap()
    );
    let (n2, gn2) = sampled(&mut page, 3, 4, false);
    assert!(
        page.animation_presented(SURFACE, gn2, frame_time(4))
            .unwrap()
    );
    let (n3, _) = sampled(&mut page, 4, 5, true);
    // Starts where it was and moves one frame's worth (255 / 12) per frame.
    assert_eq!(n, 0);
    for (a, b) in [(n, n1), (n1, n2), (n2, n3)] {
        assert!((18..=25).contains(&(b - a)), "{n} {n1} {n2} {n3}");
    }
}

#[test]
fn a_visibility_flip_drops_the_waiting_frame_a_frame_ahead_would_build_on() {
    let mut page = page(
        transition(&["opacity"], 200),
        "backgroundColor:'#000000'",
        "opacity:0",
        "''",
    );
    let (display, _) = present(&mut page, frame_time(0));
    press(&mut page, &display, true);
    let sampled = |page: &mut ViewSession, now: u64, ahead: bool| {
        page.prepare_view_sampled(
            200,
            100,
            1.0,
            false,
            None,
            frame_time(now),
            frame_time(now + 1),
            ahead,
            |display| Ok(shoot(display, false)),
        )
        .map(|_| page.generation())
    };
    let waiting = sampled(&mut page, 1, false).unwrap();
    page.set_animation_target(Some(SURFACE), false, frame_time(2))
        .unwrap();
    page.set_animation_target(Some(SURFACE), true, frame_time(3))
        .unwrap();
    // The flip invalidated the waiting frame: a frame ahead starts fresh instead of failing.
    let fresh = sampled(&mut page, 3, true).unwrap();
    assert!(
        !page
            .animation_presented(SURFACE, waiting, frame_time(4))
            .unwrap()
    );
    assert!(
        page.animation_presented(SURFACE, fresh, frame_time(4))
            .unwrap()
    );
    // A hidden page's catch-up frames park time, so a frame ahead of one starts fresh too.
    page.set_animation_target(Some(SURFACE), false, frame_time(5))
        .unwrap();
    sampled(&mut page, 5, false).unwrap();
    sampled(&mut page, 5, true).unwrap();
}

#[test]
fn a_focus_change_on_the_scroll_path_composes_a_frame_ahead_once() {
    use std::collections::HashMap;
    let spin = r#"{"name":"spin","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":"linear","opacity":null,"transforms":[{"kind":"rotate","angle":[{"offset":0,"value":0},{"offset":1,"value":360}]}]}"#;
    let module = super::source(&format!(
        "configureClasses([{{name:'base',when:{{}},style:{{backgroundColor:'#ffffff'}}}},{{name:'focused',when:{{state:'focus'}},style:{{backgroundColor:'#0000ff'}}}}]);function render(){{const draft=useLocal('draft','typed');return h('scroll',{{style:{{height:100}}}},h('column',{{style:{{width:20,height:20,backgroundColor:'#00ff00',animation:{spin}}}}}),h('view',{{style:{{height:300}}}}),h('input',{{'data-testid':'i',className:'base focused',autoFocus:true,bind:draft,style:{{width:150}}}}))}}"
    ));
    let mut page = ViewSession::new(&module).unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let sampled = |page: &mut ViewSession,
                   at: u64,
                   rebuild: bool,
                   offsets: Option<&HashMap<String, f32>>,
                   ahead: bool| {
        page.prepare_view_sampled(
            200,
            100,
            1.0,
            rebuild,
            offsets,
            frame_time(at),
            frame_time(at + 1),
            ahead,
            |display| Ok(display.nodes.clone()),
        )
    };
    let nodes = sampled(&mut page, 0, true, None, false).unwrap();
    let scroll = nodes[0].scroll.as_ref().unwrap().id.clone();
    // A scroll-only frame waits; the next, prepared ahead of it, scrolls the autofocus input
    // into view, so focus moves while it composes and it composes again on the full path.
    sampled(&mut page, 1, false, None, false).unwrap();
    let down = HashMap::from([(scroll, 400.0)]);
    sampled(&mut page, 1, false, Some(&down), true).unwrap();
}
