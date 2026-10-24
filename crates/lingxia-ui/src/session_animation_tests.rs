use super::*;
use crate::animation::{
    Definition, Effect, Timing, Wakeup,
    interpolate::{Easing, Keyframe, Track},
};
use std::num::NonZeroU32;

const CHART: &str = "/i:0:0";

fn authored_effect() -> String {
    serde_json::json!({"name":"enter","durationMs":100,"delayMs":0,"iterations":1,
        "easing":"linear","opacity":[{"offset":0,"value":0},{"offset":1,"value":1}],"transforms":[]}).to_string()
}

#[test]
fn authored_transform_can_bring_an_offscreen_node_into_the_animation_visible_set() {
    let effect = serde_json::json!({"name":"slide","durationMs":100,"delayMs":0,"iterations":1,"easing":"linear",
      "transforms":[{"kind":"translate","x":[{"offset":0,"value":0},{"offset":1,"value":0}],
        "y":[{"offset":0,"value":-40},{"offset":1,"value":0}]}]});
    let source = tests::source(&format!(
        "function render(){{return h('column',{{}},h('spacer',{{style:{{height:130,shrink:0}}}}),h('column',{{testId:'incoming',style:{{height:20,shrink:0,background:'#ff0000',animation:{effect}}}}}));}}"
    ));
    let mut page = ViewSession::new(&source).unwrap();
    page.set_animation_surface(Some(1), ms(0)).unwrap();
    let first = automatic(&mut page, 0, true, 0.0);
    let node = first
        .nodes
        .iter()
        .find(|n| n.test_id == "incoming")
        .unwrap();
    assert_eq!(node.bounds[1], 90.0);
    assert!(page.animation_visible.contains(&node.identity));
    let pixels = first.rasterize().unwrap();
    assert_eq!(
        &pixels.pixels.data_as_u8_slice()[(95 * 200 + 10) * 4..(95 * 200 + 10) * 4 + 4],
        &[255, 0, 0, 255]
    );
    page.animation_presented(1, page.generation(), ms(0))
        .unwrap();
    assert_eq!(
        page.view_animation_wakeup(ms(10)).unwrap(),
        Wakeup::NextFrame
    );
    let last = automatic(&mut page, 110, false, 0.0);
    assert_eq!(
        last.nodes
            .iter()
            .find(|n| n.test_id == "incoming")
            .unwrap()
            .bounds[1],
        130.0
    );
    page.animation_presented(1, page.generation(), ms(110))
        .unwrap();
    assert_eq!(page.view_animation_wakeup(ms(120)).unwrap(), Wakeup::None);
}

fn automatic(page: &mut ViewSession, time: u64, rebuild: bool, offset: f32) -> DisplayList {
    page.prepare_view_animated(
        200,
        120,
        1.0,
        rebuild,
        Some(&HashMap::from([("root".into(), offset)])),
        ms(time),
        Ok,
    )
    .unwrap()
}

#[test]
fn authored_collection_uses_records_and_rolls_back_failed_pixel_builds() {
    let source = tests::source(&format!(
        "globalThis.effect={};function render(){{return h('column',{{style:{{width:40,height:40,background:'#ff0000',animation:effect}}}});}}",
        authored_effect()
    ));
    let mut page = ViewSession::new(&source).unwrap();
    page.set_animation_surface(Some(1), ms(0)).unwrap();
    let first = automatic(&mut page, 0, true, 0.0);
    assert_eq!(
        first.rasterize().unwrap().pixels.data_as_u8_slice()[0..4],
        [246, 247, 251, 255]
    );
    page.animation_presented(1, page.generation(), ms(0))
        .unwrap();
    automatic(&mut page, 50, false, 0.0);
    assert_eq!(page.frame_metrics().view_passes, 0);
    assert_eq!(page.frame_metrics().layout_passes, 0);
    assert_eq!(
        page.pending_animation.as_ref().unwrap().2.visuals()[CHART].opacity,
        Some(0.5)
    );
    let generation = page.generation();
    page.eval("effect={...effect,name:'replacement'}").unwrap();
    assert!(
        page.prepare_view_animated(200, 120, 1.0, true, None, ms(60), |_| Err::<(), _>(
            "failed pixels".into()
        ))
        .is_err()
    );
    assert_eq!(page.generation(), generation);
    assert!(page.animation_presented(1, generation, ms(60)).unwrap());
    assert_eq!(
        page.animation_inventory
            .definitions()
            .next()
            .unwrap()
            .1
            .name,
        "enter"
    );
    page.eval("effect=null").unwrap();
    automatic(&mut page, 70, true, 0.0);
    page.animation_presented(1, page.generation(), ms(70))
        .unwrap();
    assert_eq!(page.animation_inventory.definitions().count(), 0);
    assert_eq!(page.view_animation_wakeup(ms(71)).unwrap(), Wakeup::None);
}

#[test]
fn authored_virtual_animation_retains_eviction_but_restarts_after_semantic_removal() {
    let source = tests::source(&format!(
        "globalThis.items=Array.from({{length:100}},(_,i)=>'item-'+i);globalThis.effect={};function render(){{return h('list',{{style:{{height:120}},count:items.length,itemKeys:items,height:()=>40,row:i=>h('column',{{key:items[i],testId:items[i],style:{{height:40,background:'#ff0000',...(items[i]==='item-1'?{{animation:effect}}:{{}})}}}})}});}}",
        authored_effect()
    ));
    let mut page = ViewSession::new(&source).unwrap();
    page.set_animation_surface(Some(1), ms(0)).unwrap();
    let first = automatic(&mut page, 0, true, 0.0);
    let id = first
        .nodes
        .iter()
        .find(|node| node.test_id == "item-1")
        .unwrap()
        .identity
        .clone();
    page.animation_presented(1, page.generation(), ms(0))
        .unwrap();
    page.eval("items.unshift('inserted')").unwrap();
    automatic(&mut page, 50, true, 0.0);
    assert_eq!(
        page.pending_animation.as_ref().unwrap().2.visuals()[&id].opacity,
        Some(0.5)
    );
    page.animation_presented(1, page.generation(), ms(50))
        .unwrap();
    let away = automatic(&mut page, 60, false, 2000.0);
    assert!(!away.nodes.iter().any(|node| node.identity == id));
    page.animation_presented(1, page.generation(), ms(60))
        .unwrap();
    assert_eq!(page.animation_inventory.definitions().count(), 1);
    assert_eq!(page.view_animation_wakeup(ms(70)).unwrap(), Wakeup::None);
    automatic(&mut page, 110, false, 0.0);
    assert!(
        page.pending_animation
            .as_ref()
            .unwrap()
            .2
            .visuals()
            .is_empty()
    );
    page.animation_presented(1, page.generation(), ms(110))
        .unwrap();
    automatic(&mut page, 120, false, 2000.0);
    page.animation_presented(1, page.generation(), ms(120))
        .unwrap();
    page.eval("items=items.filter(key=>key!=='item-1')")
        .unwrap();
    assert!(
        page.prepare_view_animated(200, 120, 1.0, true, None, ms(130), |_| Err::<(), _>(
            "failed pixels".into()
        ))
        .is_err()
    );
    assert_eq!(page.animation_inventory.definitions().count(), 1);
    automatic(&mut page, 140, true, 2000.0);
    page.animation_presented(1, page.generation(), ms(140))
        .unwrap();
    assert_eq!(page.animation_inventory.definitions().count(), 0);
    page.eval("items.splice(1,0,'item-1')").unwrap();
    automatic(&mut page, 150, true, 0.0);
    assert_eq!(
        page.pending_animation.as_ref().unwrap().2.visuals()[&id].opacity,
        Some(0.0)
    );
}

#[test]
fn authored_virtual_effect_requires_inventory_and_unseen_overscan_does_not_start() {
    let source = tests::source(&format!(
        "globalThis.keys=undefined;function render(){{return h('list',{{style:{{height:120}},count:100,itemKeys:keys,height:()=>40,row:i=>h('column',{{key:'item-'+i,testId:'item-'+i,style:{{height:40,...(i===4?{{animation:{}}}:{{}})}}}})}});}}",
        authored_effect()
    ));
    let mut page = ViewSession::new(&source).unwrap();
    page.set_animation_surface(Some(1), ms(0)).unwrap();
    assert!(
        page.prepare_view_animated(200, 120, 1.0, true, None, ms(0), Ok)
            .err()
            .unwrap()
            .contains("require itemKeys")
    );
    assert_eq!(page.generation(), 0);
    page.eval("keys=Array.from({length:100},(_,i)=>'item-'+i)")
        .unwrap();
    let first = automatic(&mut page, 0, true, 0.0);
    let id = first
        .nodes
        .iter()
        .find(|node| node.test_id == "item-4")
        .unwrap()
        .identity
        .clone();
    assert!(!page.animation_visible.contains(&id));
    page.animation_presented(1, page.generation(), ms(0))
        .unwrap();
    assert_eq!(page.view_animation_wakeup(ms(500)).unwrap(), Wakeup::None);
    automatic(&mut page, 500, false, 160.0);
    assert_eq!(
        page.pending_animation.as_ref().unwrap().2.visuals()[&id].opacity,
        Some(0.0)
    );
}

fn ms(time: u64) -> Duration {
    Duration::from_millis(time)
}

fn definitions() -> [(String, Definition); 1] {
    [(
        CHART.into(),
        Definition {
            name: "chart".into(),
            timing: Timing::new(ms(100), ms(0), NonZeroU32::new(1)).unwrap(),
            effect: Effect::new(
                Some(
                    Track::new(
                        vec![
                            Keyframe {
                                offset: 0.0,
                                value: 0.0,
                            },
                            Keyframe {
                                offset: 1.0,
                                value: 1.0,
                            },
                        ],
                        Easing::LINEAR,
                    )
                    .unwrap(),
                ),
                vec![],
            )
            .unwrap(),
        },
    )]
}

fn page() -> ViewSession {
    let mut page = ViewSession::new(&tests::source(
        "function render(){return h('column',{},h('text',{},'Chart'));}",
    ))
    .unwrap();
    page.set_animation_surface(Some(10), ms(0)).unwrap();
    page
}

fn prepare(page: &mut ViewSession, now: u64, rebuild: bool) -> Option<f64> {
    page.prepare_animated(
        200,
        100,
        1.0,
        rebuild,
        None,
        ms(now),
        definitions(),
        [CHART],
        |display| {
            assert_eq!(display.nodes.len(), 2);
            assert_eq!(display.nodes[0].identity, CHART);
            Ok(())
        },
    )
    .unwrap();
    page.pending_animation
        .as_ref()
        .unwrap()
        .2
        .visuals()
        .get(CHART)
        .and_then(|visual| visual.opacity)
}

#[test]
fn preparation_does_not_start_time_and_animation_frames_reuse_layout() {
    let mut page = page();
    assert_eq!(prepare(&mut page, 0, true), Some(0.0));
    assert_eq!(prepare(&mut page, 500, true), Some(0.0));
    let generation = page.generation();
    assert_eq!(
        page.animation_wakeup(ms(500), [CHART]).unwrap(),
        Wakeup::None
    );
    assert!(page.animation_presented(10, generation, ms(600)).unwrap());
    assert_eq!(prepare(&mut page, 650, false), Some(0.5));
    assert_eq!(page.frame_metrics().view_passes, 0);
    assert_eq!(page.frame_metrics().layout_passes, 0);
    assert_eq!(page.frame_metrics().text_us, 0);
    assert!(
        page.animation_presented(10, page.generation(), ms(650))
            .unwrap()
    );
    assert_eq!(prepare(&mut page, 700, false), None);
    assert!(
        page.animation_presented(10, page.generation(), ms(700))
            .unwrap()
    );
    assert_eq!(
        page.animation_wakeup(ms(700), [CHART]).unwrap(),
        Wakeup::None
    );
}

#[test]
fn failed_build_and_bad_acknowledgements_preserve_the_valid_candidate() {
    let mut page = page();
    prepare(&mut page, 10, true);
    let generation = page.generation();
    let result = page.prepare_animated(
        200,
        100,
        1.0,
        true,
        None,
        ms(20),
        definitions(),
        [CHART],
        |_| Err::<(), _>("injected paint failure".into()),
    );
    assert!(result.is_err());
    assert_eq!(page.generation(), generation);
    assert!(!page.animation_presented(11, generation, ms(30)).unwrap());
    assert!(
        !page
            .animation_presented(10, generation + 1, ms(30))
            .unwrap()
    );
    assert!(page.animation_presented(10, generation, ms(9)).is_err());
    assert!(page.animation_presented(10, generation, ms(30)).unwrap());
    assert_eq!(prepare(&mut page, 80, false), Some(0.5));
}

#[test]
fn surface_replacement_and_parking_reject_old_callbacks_and_preserve_active_time() {
    let mut page = page();
    prepare(&mut page, 0, true);
    assert!(
        page.animation_presented(10, page.generation(), ms(0))
            .unwrap()
    );
    prepare(&mut page, 20, false);
    let old = page.generation();
    page.set_animation_surface(None, ms(20)).unwrap();
    assert!(!page.animation_presented(10, old, ms(50)).unwrap());
    assert_eq!(
        page.animation_wakeup(ms(500), [CHART]).unwrap(),
        Wakeup::None
    );
    page.set_animation_surface(Some(11), ms(1000)).unwrap();
    assert!((prepare(&mut page, 1030, false).unwrap() - 0.5).abs() < 1e-12);
    let current = page.generation();
    assert!(!page.animation_presented(10, old, ms(1030)).unwrap());
    assert!(page.animation_presented(11, current, ms(1030)).unwrap());
    prepare(&mut page, 1040, false);
    let replaced = page.generation();
    page.set_animation_surface(Some(12), ms(1040)).unwrap();
    assert!(!page.animation_presented(11, replaced, ms(1040)).unwrap());
    assert!((prepare(&mut page, 1050, false).unwrap() - 0.7).abs() < 1e-12);
}

#[test]
fn ordinary_frames_and_reset_cannot_acknowledge_old_animation_candidates() {
    let mut page = page();
    prepare(&mut page, 0, true);
    let old = page.generation();
    page.layout(200, 100, 1.0).unwrap();
    assert!(!page.animation_presented(10, old, ms(20)).unwrap());
    assert_eq!(prepare(&mut page, 50, false), Some(0.0));
    assert!(
        page.animation_presented(10, page.generation(), ms(50))
            .unwrap()
    );
    prepare(&mut page, 70, false);
    let old = page.generation();
    page.reset().unwrap();
    assert!(!page.animation_presented(10, old, ms(80)).unwrap());
    assert_eq!(prepare(&mut page, 100, true), Some(0.0));
}

#[test]
fn keyed_virtual_row_animation_survives_reorder_and_window_eviction() {
    let source = tests::source(
        "globalThis.items=Array.from({length:100},(_,i)=>'item-'+i);
         function render(){return h('list',{style:{height:120},count:items.length,itemKeys:items,
           height:()=>40,row:i=>h('text',{key:items[i],testId:items[i]},items[i])});}",
    );
    let mut page = ViewSession::new(&source).unwrap();
    let initial = page.layout(200, 120, 1.0).unwrap();
    assert!(
        initial.nodes[0].scroll.as_ref().unwrap().max_offset > 5000.0,
        "unrealized rows must contribute their estimated scroll extent"
    );
    let item = initial
        .nodes
        .iter()
        .find(|node| node.test_id == "item-1")
        .unwrap();
    let identity = item.identity.clone();
    let old_y = item.bounds[1];
    let scroll = initial.nodes[0].scroll.as_ref().unwrap().id.clone();
    page.set_animation_surface(Some(10), ms(0)).unwrap();
    let definition = definitions()[0].1.clone();
    let prepare = |page: &mut ViewSession, now, rebuild, offset, visible| {
        page.prepare_animated(
            200,
            120,
            1.0,
            rebuild,
            Some(&HashMap::from([(scroll.clone(), offset)])),
            ms(now),
            [(identity.clone(), definition.clone())],
            if visible {
                vec![identity.as_str()]
            } else {
                vec![]
            },
            Ok,
        )
        .unwrap_or_else(|error| panic!("frame at {now}ms: {error}"))
    };
    prepare(&mut page, 0, false, 0.0, true);
    page.animation_presented(10, page.generation(), ms(0))
        .unwrap();
    page.eval("items.splice(0,0,'inserted')").unwrap();
    let moved = prepare(&mut page, 50, true, 0.0, true);
    let item = moved
        .nodes
        .iter()
        .find(|node| node.test_id == "item-1")
        .unwrap();
    assert_eq!(item.identity, identity);
    assert!(item.bounds[1] > old_y);
    assert_eq!(
        page.pending_animation.as_ref().unwrap().2.visuals()[&identity].opacity,
        Some(0.5)
    );
    page.animation_presented(10, page.generation(), ms(50))
        .unwrap();
    let away = prepare(&mut page, 60, false, 2000.0, false);
    assert!(away.nodes[0].scroll.as_ref().unwrap().offset > 1500.0);
    assert!(!away.nodes.iter().any(|node| node.test_id == "item-1"));
    page.animation_presented(10, page.generation(), ms(60))
        .unwrap();
    let returned = prepare(&mut page, 110, false, 0.0, true);
    assert_eq!(
        returned
            .nodes
            .iter()
            .find(|node| node.test_id == "item-1")
            .unwrap()
            .identity,
        identity
    );
    assert!(
        page.pending_animation
            .as_ref()
            .unwrap()
            .2
            .visuals()
            .is_empty(),
        "completed entrance must not restart"
    );
    page.eval("items[2]='replacement'").unwrap();
    let replaced = page.layout(200, 120, 1.0).unwrap();
    assert!(!replaced.nodes.iter().any(|node| node.identity == identity));
    assert_ne!(
        replaced
            .nodes
            .iter()
            .find(|node| node.test_id == "replacement")
            .unwrap()
            .identity,
        identity
    );
}

#[test]
fn view_wire_requires_matching_definition_and_presentation_aware_rendering() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/lingxia-ui/tests/fixtures/animations.json"
    ))
    .unwrap();
    let payload = fixture["chart-in"].to_string();
    let source = tests::source(&format!(
        "function render(){{return h('column',{{style:{{width:40,height:40,background:'#ff0000',animation:{payload}}}}});}}"
    ));
    let mut page = ViewSession::new(&source).unwrap();
    let error = page.layout(80, 80, 1.0).err().unwrap();
    assert!(
        error.contains("requires animation presentation scheduling"),
        "{error}"
    );
    assert_eq!(page.generation(), 0);
    page.set_animation_surface(Some(1), ms(0)).unwrap();
    let definition = crate::animation::decode_definition(&payload).unwrap();
    let mut wrong = definition.clone();
    wrong.name = "different".into();
    assert!(
        page.prepare_animated(
            80,
            80,
            1.0,
            true,
            None,
            ms(0),
            [(CHART.into(), wrong)],
            [CHART],
            Ok
        )
        .is_err()
    );
    assert_eq!(page.generation(), 0);
    let first = page
        .prepare_animated(
            80,
            80,
            1.0,
            true,
            None,
            ms(0),
            [(CHART.into(), definition.clone())],
            [CHART],
            Ok,
        )
        .unwrap();
    assert!((first.nodes[0].bounds[2] - 38.4).abs() < 0.001);
    let initial = first.rasterize().unwrap();
    assert_eq!(
        &initial.pixels.data_as_u8_slice()[(20 * 80 + 20) * 4..(20 * 80 + 20) * 4 + 4],
        &[246, 247, 251, 255]
    );
    assert!(
        page.animation_presented(1, page.generation(), ms(0))
            .unwrap()
    );
    let next = page
        .prepare_animated(
            80,
            80,
            1.0,
            false,
            None,
            ms(175),
            [(CHART.into(), definition)],
            [CHART],
            Ok,
        )
        .unwrap();
    assert_eq!(page.frame_metrics().view_passes, 0);
    assert_eq!(page.frame_metrics().layout_passes, 0);
    let next = next.rasterize().unwrap();
    let pixel = &next.pixels.data_as_u8_slice()[(20 * 80 + 20) * 4..(20 * 80 + 20) * 4 + 4];
    assert!(pixel[0] > 250 && pixel[1] < 50 && pixel[2] < 50);
    assert!(
        page.prepare_native(80, 80, 1.0, false, None, Ok)
            .err()
            .unwrap()
            .contains("requires animation presentation scheduling")
    );
}
