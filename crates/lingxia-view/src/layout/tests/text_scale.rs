use serde_json::json;

#[test]
fn scaling_reflows_text_and_input_once_and_invalidates_retained_geometry() {
    let module = crate::session::tests::source(
        "function render({env}){if(env.textScale)throw Error('internal mapping leaked');return h('view',{style:{fontSize:16,lineHeight:'24px'}},h('input',{value:'Scaling',style:{minHeight:44,padding:8}}),h('view',{},h('input',{value:'Scaling',style:{minHeight:44,padding:8}})),h('text',{},'Wrapping text for larger fonts in a narrow viewport'))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let baseline = page.layout(220, 600, 1.0).unwrap();
    let mut environment = json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"textScale":[[0,0],[16384,32768]]});
    page.set_environment(environment.clone()).unwrap();
    let enlarged = page.layout(220, 600, 1.0).unwrap();
    let first = &enlarged.retained.inputs[1].as_ref().unwrap().layout;
    let nested = &enlarged.retained.inputs[3].as_ref().unwrap().layout;
    assert_eq!(first.height(), 48.0);
    assert_eq!(nested.height(), 48.0);
    assert!((first.width() - nested.width()).abs() < 0.01);
    assert!(enlarged.nodes[4].bounds[3] > baseline.nodes[4].bounds[3]);
    assert_eq!(baseline.nodes[1].bounds[3], 44.0);
    assert!(enlarged.nodes[1].bounds[3] >= 64.0);
    environment["textScale"] = json!([[0, 0], [16, -1]]);
    assert!(page.set_environment(environment.clone()).is_err());
    let preserved = page.layout(220, 600, 1.0).unwrap();
    assert_eq!(
        preserved.retained.inputs[1]
            .as_ref()
            .unwrap()
            .layout
            .height(),
        48.0
    );
    environment.as_object_mut().unwrap().remove("textScale");
    page.set_environment(environment).unwrap();
    let restored = page.layout(220, 600, 1.0).unwrap();
    assert_eq!(restored.nodes[4].bounds, baseline.nodes[4].bounds);
    assert_eq!(
        restored.retained.inputs[1]
            .as_ref()
            .unwrap()
            .layout
            .height(),
        24.0
    );
}

#[test]
fn a_select_chevron_and_its_room_grow_with_the_text_to_one_and_a_half() {
    use crate::scene::paint::Paint;
    let module = crate::session::tests::source(
        "function render(){return h('view',{style:{alignItems:'flex-start'}},h('select',{'data-testid':'s','aria-label':'Plan',options:[{value:'m',label:'Monthly'}],style:{fontSize:16}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let chevron = |page: &mut crate::ViewSession| {
        let display = page.layout(300, 200, 1.0).unwrap();
        let select = display.nodes.iter().find(|n| n.test_id == "s").unwrap();
        let [x, _, w, _] = select.bounds;
        let points = display
            .commands
            .iter()
            .find_map(|paint| match paint {
                Paint::Stroke { points, .. } if points.len() == 3 => Some(points.clone()),
                _ => None,
            })
            .unwrap();
        // Chevron width and its centre's distance from the select's end.
        (points[2][0] - points[0][0], x + w - points[1][0])
    };
    let (narrow, inset) = chevron(&mut page);
    assert_eq!(narrow, 10.0);
    let mut environment = json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"textScale":[[0,0],[16384,32768]]});
    page.set_environment(environment.clone()).unwrap();
    let (grown, grown_inset) = chevron(&mut page);
    // Text doubles; the chevron stops at 1.5×.
    assert_eq!(grown, 15.0);
    assert_eq!(grown_inset - inset, 5.0);
    environment.as_object_mut().unwrap().remove("textScale");
    page.set_environment(environment).unwrap();
    assert_eq!(chevron(&mut page).0, 10.0);
}
