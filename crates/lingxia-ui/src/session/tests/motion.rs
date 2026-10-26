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
        "function render(){return h('page',{style:{backgroundColor:'#ffffff'}},h('button',{testId:'b',onClick(){},className:'active:scale-95',style:{position:'absolute',left:0,top:0,width:100,height:40,padding:0,borderRadius:0,backgroundColor:'#000000'}},''))}",
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
fn environments_default_reduced_motion_off_and_reject_bad_values() {
    let module =
        super::source("function render({env}){return h('text',{},String(env.reducedMotion))}");
    let mut page = ViewSession::new(&module).unwrap();
    let mut environment = json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"runner":false});
    page.set_environment(environment.clone()).unwrap();
    assert_eq!(page.layout(100, 50, 1.0).unwrap().nodes[0].text, "false");
    environment["reducedMotion"] = json!(true);
    page.set_environment(environment.clone()).unwrap();
    assert_eq!(page.layout(100, 50, 1.0).unwrap().nodes[0].text, "true");
    environment["reducedMotion"] = json!("yes");
    assert!(page.set_environment(environment).is_err());
}
