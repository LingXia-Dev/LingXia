//! Lines inside a lifted surface (a dialog, a menu) take that surface's pair, so they keep the
//! contrast they have on the page.
use crate::ViewSession;
use crate::scene::paint::Paint;
use lingxia_theme::Role;
use serde_json::json;

#[test]
fn lines_inside_a_lifted_surface_take_its_pair_until_another_surface() {
    let module = super::source(
        "function render(){const b=(w,c)=>h('view',{style:{width:w,height:10,borderWidth:1,borderColor:c||'line'}});return h('page',{},b(10),h('view',{style:{backgroundColor:'surface-overlay'}},b(20),h('view',{style:{backgroundColor:'accent'}},b(30)),h('view',{style:{backgroundColor:'surface'}},b(40))),h('view',{style:{backgroundColor:'surface-raised/90'}},b(50,'line-strong/50')))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_environment(json!({"theme":"dark","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en","canDock":false}))
        .unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    let border = |width: f32| {
        display
            .commands
            .iter()
            .find_map(|paint| match paint {
                Paint::Border { color, bounds, .. } if (bounds[2] - width).abs() < 0.5 => {
                    Some(*color)
                }
                _ => None,
            })
            .unwrap()
    };
    let role = |role| page.renderer.palette.role(role);
    assert_ne!(
        role(Role::Line),
        role(Role::LineOnOverlay),
        "dark lifts the pair"
    );
    assert_eq!(border(10.0), role(Role::Line));
    assert_eq!(border(20.0), role(Role::LineOnOverlay));
    assert_eq!(
        border(30.0),
        role(Role::LineOnOverlay),
        "a filled control keeps its surface's lines"
    );
    assert_eq!(border(40.0), role(Role::Line), "a surface resets them");
    let [r, g, b, a] = role(Role::LineStrongOnRaised);
    assert_eq!(
        border(50.0),
        [r, g, b, ((u32::from(a) * 50 + 50) / 100) as u8],
        "the authored alpha stays"
    );
}

#[test]
fn a_page_cannot_name_an_internal_role() {
    assert!(crate::theme::parse("line-on-overlay").is_err());
    assert!(crate::theme::parse("line-strong-on-raised/50").is_err());
}

#[test]
fn a_line_entering_inside_an_overlay_eases_between_the_overlay_pair() {
    use crate::session::lifecycle::SurfaceTicket;
    use crate::{DisplayList, animation::Wakeup};
    use std::time::Duration;
    const SURFACE: SurfaceTicket = SurfaceTicket::new(1);
    let panel = "h('view',{className:'starting:edge',style:{width:50,height:20,borderWidth:2,borderColor:'line',transition:{properties:['border'],durationMs:200,delayMs:0,easing:'linear'}}})";
    let module = super::source(&format!(
        "function render(){{const shown=useLocal('shown',false);return h('page',{{}},h('button',{{'data-testid':'toggle',onClick(){{shown.set(true)}},style:{{width:40,height:20}}}},'T'),h('view',{{style:{{backgroundColor:'surface-overlay',padding:10}}}},shown.value && {panel}))}}"
    ))
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'starting:edge',when:{state:'starting'},style:{borderColor:'line-strong'}}]);__lingxiaPublish(render);",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.set_environment(json!({"theme":"dark","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","language":"en","canDock":false}))
        .unwrap();
    page.set_animation_surface(Some(SURFACE), Duration::ZERO)
        .unwrap();
    let at = |index: u64| Duration::from_micros(index * 16_667);
    let present = |page: &mut ViewSession, index: u64, rebuild: bool| {
        let (border, toggle) = page
            .prepare_view_animated(
                200,
                100,
                1.0,
                rebuild,
                None,
                at(index),
                |display: DisplayList| {
                    let border = display.commands.iter().find_map(|paint| match paint {
                        Paint::Border { color, bounds, .. } if (bounds[2] - 50.0).abs() < 0.5 => {
                            Some(*color)
                        }
                        _ => None,
                    });
                    let toggle = display
                        .nodes
                        .iter()
                        .find(|n| n.test_id == "toggle")
                        .and_then(|n| n.on_click);
                    Ok((border, toggle))
                },
            )
            .unwrap();
        assert!(
            page.animation_presented(SURFACE, page.generation(), at(index))
                .unwrap()
        );
        (
            border,
            toggle,
            page.view_animation_wakeup(at(index)).unwrap(),
        )
    };
    let (_, toggle, _) = present(&mut page, 0, true);
    assert!(page.click(page.generation(), toggle.unwrap()).unwrap());
    let mut samples = Vec::new();
    for index in 1..30 {
        let (border, _, wakeup) = present(&mut page, index, index == 1);
        samples.push(border.expect("the panel's border"));
        if wakeup == Wakeup::None {
            break;
        }
    }
    let role = |role| page.renderer.palette.role(role);
    let (from, to) = (role(Role::LineStrongOnOverlay), role(Role::LineOnOverlay));
    assert_ne!(from, role(Role::LineStrong), "dark lifts the pair");
    assert_eq!(samples[0], from, "it enters from the overlay's line-strong");
    assert_eq!(
        *samples.last().unwrap(),
        to,
        "and lands on the overlay's line"
    );
    for sample in &samples {
        for channel in 0..4 {
            let (low, high) = (
                from[channel].min(to[channel]),
                from[channel].max(to[channel]),
            );
            assert!(
                (low..=high).contains(&sample[channel]),
                "{sample:?} stays between {from:?} and {to:?}"
            );
        }
    }
    assert!(
        samples
            .iter()
            .any(|sample| *sample != from && *sample != to),
        "it eases: {samples:?}"
    );
}
