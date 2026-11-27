//! The compiled half of motion: transition records, pressed transforms, reduced motion.
use crate::ViewSession;
use crate::protocol::{decode, field, tag};
use serde_json::json;

#[test]
fn transition_records_are_bounded_on_decode() {
    let wire = |value: serde_json::Value| {
        decode(
            json!([
                tag::COLUMN,
                -1,
                "",
                "",
                "",
                1,
                field::TRANSITION,
                value.to_string()
            ])
            .as_array()
            .unwrap(),
        )
    };
    let valid = json!({"properties":["opacity","transform"],"durationMs":200,"delayMs":0,"easing":[0.4,0,0.2,1]});
    assert!(wire(valid.clone()).is_ok());
    for broken in [
        json!({"properties":["height"],"durationMs":200,"delayMs":0,"easing":"linear"}),
        json!({"properties":[],"durationMs":200,"delayMs":0,"easing":"linear"}),
        json!({"properties":["opacity","opacity"],"durationMs":200,"delayMs":0,"easing":"linear"}),
        json!({"properties":["opacity"],"durationMs":10001,"delayMs":0,"easing":"linear"}),
        json!({"properties":["opacity"],"durationMs":0,"delayMs":0,"easing":"linear"}),
        json!({"properties":["opacity"],"durationMs":200,"delayMs":0,"easing":"bounce"}),
        json!({"properties":["opacity"],"durationMs":200,"delayMs":0}),
    ] {
        assert!(
            wire(broken.clone()).unwrap_err().contains("UI_TRANSITION"),
            "{broken}"
        );
    }
}

#[test]
fn pressed_states_may_scale_through_the_paint_delta() {
    let module = super::source(
        "function render(){return h('page',{style:{backgroundColor:'#ffffff'}},h('button',{'data-testid':'b',onClick(){},className:'active:scale-95',style:{position:'absolute',left:0,top:0,width:100,height:40,padding:0,borderRadius:0,backgroundColor:'#000000'}},''))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'active:scale-95',when:{state:'active'},style:{scale:[0.5,0.5]}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(200, 100, 1.0).unwrap();
    let button = display.nodes.iter().find(|n| n.test_id == "b").unwrap();
    assert!(button.pressed_style);
    let identity = button.identity.clone();
    page.set_pointer_state(Some(&identity), None);
    let pressed = page.layout(200, 100, 1.0).unwrap();
    let bounds = pressed
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .bounds;
    assert_eq!(bounds, [25.0, 10.0, 50.0, 20.0]);
}

#[test]
fn a_pressed_switch_paints_its_track_not_a_box() {
    use crate::scene::paint::Paint;
    let module = super::source(
        // Off: an on track deepens to the pressed solid instead.
        "function render(){const on=useLocal('on',false);return h('page',{},h('switch',{'data-testid':'s',bind:on,'aria-label':'Wi-Fi'}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(400, 200, 1.0).unwrap();
    let switch = display.nodes.iter().find(|n| n.test_id == "s").unwrap();
    assert!(
        switch.pressed_style,
        "the switch repaints itself, so no platform overlay covers its touch box"
    );
    let [x, y, w, h] = switch.bounds;
    let identity = switch.identity.clone();
    let layer =
        crate::interaction::chrome::FeedbackStyle::press_layer(&page.renderer.palette, false);
    let layers = |display: &crate::scene::display::DisplayList| -> Vec<[f32; 4]> {
        display
            .commands
            .iter()
            .filter_map(|p| match p {
                Paint::Rect { color, bounds, .. } if *color == layer => Some(*bounds),
                _ => None,
            })
            .collect()
    };
    assert!(layers(&display).is_empty());
    // The track: 52x32 at the inline end, vertically centred in the 12px padding.
    let track = [x + w - 12.0 - 52.0, y + h / 2.0 - 16.0, 52.0, 32.0];
    let track_fill = |display: &crate::scene::display::DisplayList| {
        display.commands.iter().find_map(|p| match p {
            Paint::Rect { color, bounds, .. } if *bounds == track => Some(*color),
            _ => None,
        })
    };
    let resting = track_fill(&display).unwrap();
    page.set_pointer_state(Some(&identity), None);
    let pressed = page.layout(400, 200, 1.0).unwrap();
    // The default off track is opaque: it deepens, as any opaque fill does, and takes no layer.
    let deep = track_fill(&pressed).unwrap();
    assert!(
        layers(&pressed).is_empty(),
        "no state layer over an opaque track"
    );
    assert!(
        deep[3] == 255 && (0..3).all(|i| deep[i] < resting[i]),
        "the pressed track is deeper: {resting:?} -> {deep:?}"
    );
    assert!(
        !pressed.commands.iter().any(|p| matches!(p,
            Paint::Rect { color, bounds, .. } if color[3] == 255 && *bounds == [x, y, w, h])),
        "no opaque fill covers the switch's box"
    );
    page.set_pointer_state(None, None);
    assert!(layers(&page.layout(400, 200, 1.0).unwrap()).is_empty());
}

#[test]
fn a_focused_switch_and_checkbox_ring_their_indicator_not_their_box() {
    use crate::scene::paint::Paint;
    let module = super::source(
        "function render(){const on=useLocal('on',true);return h('page',{},h('switch',{'data-testid':'s',bind:on,'aria-label':'Wi-Fi'}),h('checkbox',{'data-testid':'c',bind:on},'Agree'))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(400, 200, 1.0).unwrap();
    let focus = page.renderer.palette.role(lingxia_theme::Role::Focus);
    let rings = |display: &crate::scene::display::DisplayList| -> Vec<([f32; 4], f32)> {
        display
            .commands
            .iter()
            .filter_map(|p| match p {
                Paint::Border {
                    color,
                    bounds,
                    radius,
                    ..
                } if *color == focus => Some((*bounds, *radius)),
                _ => None,
            })
            .collect()
    };
    assert!(rings(&display).is_empty());
    let node = |id: &str| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .clone()
    };
    let (switch, checkbox) = (node("s"), node("c"));
    assert!(
        switch.focus_style && checkbox.focus_style,
        "both ring themselves"
    );
    page.set_control_focus(Some(&switch.identity)).unwrap();
    let [x, y, w, h] = switch.bounds;
    // The track (52x32 at the inline end) grown by 4 on each side.
    assert_eq!(
        rings(&page.layout(400, 200, 1.0).unwrap()),
        [(
            [x + w - 12.0 - 52.0 - 4.0, y + h / 2.0 - 20.0, 60.0, 40.0],
            20.0
        )]
    );
    page.set_control_focus(Some(&checkbox.identity)).unwrap();
    let [x, y, _, h] = checkbox.bounds;
    let ring = rings(&page.layout(400, 200, 1.0).unwrap());
    assert_eq!(ring.len(), 1);
    assert_eq!(ring[0].0[2..], [32.0, 32.0], "the 24 px box grown by 4");
    assert!(ring[0].0[0] > x - 4.0 && (ring[0].0[1] + 16.0 - (y + h / 2.0)).abs() < 1.0);
    page.set_control_focus(None).unwrap();
    assert!(rings(&page.layout(400, 200, 1.0).unwrap()).is_empty());
}

#[test]
fn an_authored_focus_visible_style_replaces_a_switchs_built_in_ring() {
    use crate::scene::paint::Paint;
    let module = super::source(
        "function render(){const on=useLocal('on',true);return h('page',{},h('switch',{'data-testid':'s',bind:on,'aria-label':'Wi-Fi',className:'fv:x'}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'fv:x',when:{state:'focusVisible'},style:{backgroundColor:'#00ff00'}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(400, 200, 1.0).unwrap();
    let focus = page.renderer.palette.role(lingxia_theme::Role::Focus);
    let switch = display
        .nodes
        .iter()
        .find(|n| n.test_id == "s")
        .unwrap()
        .clone();
    assert!(switch.focus_visible_style, "the author styled its focus");
    page.set_control_focus(Some(&switch.identity)).unwrap();
    let display = page.layout(400, 200, 1.0).unwrap();
    assert!(
        !display
            .commands
            .iter()
            .any(|p| matches!(p, Paint::Border { color, .. } if *color == focus)),
        "no built-in track ring under the author's own focus style"
    );
}

#[test]
fn group_pressed_paint_applies_to_the_pressed_controls_descendants() {
    let module = super::source(
        "function render(){return h('page',{},h('button',{'data-testid':'b',onClick(){},className:'group',style:{width:100,height:40,padding:0}},h('view',{'data-testid':'disc',className:'group-active:opacity-50',style:{width:20,height:20}})),h('view',{'data-testid':'outside',className:'group-active:opacity-50',style:{width:20,height:20}}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'group',when:{},style:{}},{name:'group-active:opacity-50',when:{state:'groupActive'},style:{opacity:0.5}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(200, 100, 1.0).unwrap();
    let button = display.nodes.iter().find(|n| n.test_id == "b").unwrap();
    assert!(button.pressed_style, "a group control repaints on press");
    let identity = button.identity.clone();
    page.set_pointer_state(Some(&identity), None);
    let pressed = page.layout(200, 100, 1.0).unwrap();
    let opacity = |id: &str| {
        pressed.retained.opacities[pressed.nodes.iter().position(|n| n.test_id == id).unwrap()]
    };
    assert_eq!(opacity("disc"), 0.5);
    assert_eq!(opacity("outside"), 1.0);
}

#[test]
fn environments_default_reduced_motion_off_and_reject_bad_values() {
    let module =
        super::source("function render({env}){return h('text',{},String(env.reducedMotion))}");
    let mut page = ViewSession::new(&module).unwrap();
    let mut environment = json!({"appearance":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en","canDock":false});
    page.set_environment(environment.clone()).unwrap();
    assert_eq!(page.layout(100, 50, 1.0).unwrap().nodes[0].text, "false");
    environment["reducedMotion"] = json!(true);
    page.set_environment(environment.clone()).unwrap();
    assert_eq!(page.layout(100, 50, 1.0).unwrap().nodes[0].text, "true");
    environment["reducedMotion"] = json!("yes");
    assert!(page.set_environment(environment).is_err());
}

#[test]
fn the_animator_duration_scale_defaults_to_one_and_zero_reduces_motion() {
    let module = super::source(
        "function render({env}){return h('text',{},env.animatorDurationScale+'|'+env.reducedMotion)}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let mut environment = json!({"appearance":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en","canDock":false});
    let mut shown = |environment: &serde_json::Value| {
        page.set_environment(environment.clone())
            .map(|()| page.layout(100, 50, 1.0).unwrap().nodes[0].text.clone())
    };
    assert_eq!(shown(&environment).unwrap(), "1|false");
    environment["animatorDurationScale"] = json!(0.5);
    assert_eq!(shown(&environment).unwrap(), "0.5|false");
    environment["animatorDurationScale"] = json!(0);
    assert_eq!(shown(&environment).unwrap(), "0|true");
    for bad in [json!(-1), json!(10.5), json!("1")] {
        environment["animatorDurationScale"] = bad;
        assert!(shown(&environment).is_err());
    }
}

#[test]
fn a_pressed_slider_draws_its_active_parts() {
    use crate::scene::paint::Paint;
    let module = super::source(
        "function render(){const v=useLocal('v',50);return h('page',{},h('slider',{'data-testid':'s',bind:v,'aria-label':'Seek',className:'thin active:track:thick'}))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'thin',when:{part:'track'},style:{height:3}},{name:'active:track:thick',when:{state:'active',part:'track'},style:{height:6}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(400, 200, 1.0).unwrap();
    let slider = display.nodes.iter().find(|n| n.test_id == "s").unwrap();
    assert!(slider.pressed_style);
    let identity = slider.identity.clone();
    let line = page.renderer.palette.role(lingxia_theme::Role::Line);
    let track = |display: &crate::scene::display::DisplayList| {
        display.commands.iter().find_map(|p| match p {
            Paint::Rect { color, bounds, .. } if *color == line => Some(bounds[3]),
            _ => None,
        })
    };
    assert_eq!(track(&display), Some(3.0));
    page.set_pointer_state(Some(&identity), None);
    page.layout(400, 200, 1.0).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let pressed = page.layout(400, 200, 1.0).unwrap();
    assert_eq!(
        track(&pressed),
        Some(6.0),
        "held, the track takes its active height"
    );
}
