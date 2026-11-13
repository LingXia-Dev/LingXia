//! An authored height wins over a single-line control's default padding and touch height.

#[test]
fn fields_and_selects_take_the_height_their_author_gives_them() {
    // Liuli's field: a row box with a min-height, the editor stretched inside it without padding;
    // Liuli's select: the select itself is the box. Small, medium and large.
    let sizes = [(32, 13, 18), (40, 15, 22), (48, 16, 24)];
    let rows: Vec<String> = sizes
        .iter()
        .map(|(h, font, line)| {
            format!(
                "h('view',{{'data-testid':'field-{h}',style:{{flexDirection:'row',alignItems:'center',minHeight:{h},paddingLeft:12,paddingRight:12,borderWidth:1}}}},h('input',{{'data-testid':'input-{h}',value:'2026-11-13',style:{{alignSelf:'stretch',flexGrow:1,flexBasis:0,minWidth:0,paddingLeft:0,paddingRight:0,fontSize:{font},lineHeight:'{line}px'}}}}),h('view',{{style:{{width:16,height:16}}}})),h('select',{{'data-testid':'select-{h}','aria-label':'Plan',options:[{{value:'m',label:'Monthly'}}],style:{{minHeight:{h},paddingLeft:12,paddingRight:12,borderWidth:1,fontSize:{font},lineHeight:'{line}px'}}}})"
            )
        })
        .collect();
    let module = crate::session::tests::source(&format!(
        "function render(){{return h('view',{{}},{},h('input',{{'data-testid':'bare',value:'x'}}),h('select',{{'data-testid':'bare-select','aria-label':'Plan',options:[{{value:'m',label:'Monthly'}}],style:{{fontSize:15,lineHeight:'22px'}}}}))}}",
        rows.join(",")
    ));
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(300, 800, 1.0).unwrap();
    let height = |id: &str| {
        display
            .nodes
            .iter()
            .find(|node| node.test_id == id)
            .unwrap_or_else(|| panic!("{id}"))
            .bounds[3]
    };
    for (h, _, _) in sizes {
        let h = h as f32;
        assert_eq!(height(&format!("field-{h}")), h, "field {h}");
        assert_eq!(
            height(&format!("input-{h}")),
            h - 2.0,
            "editor fills the box {h}"
        );
        assert_eq!(height(&format!("select-{h}")), h, "select {h}");
    }
    // Unsized, a control keeps its touch-friendly defaults.
    assert_eq!(height("bare"), 48.0);
    assert_eq!(height("bare-select"), 12.0 + 22.0 + 12.0);
}
