//! `ch` widths: one rule for width, min-width and max-width, measured in the element's own digits.

#[test]
fn width_min_width_and_max_width_take_ch_in_the_elements_own_digits() {
    let digits =
        "fontSize:15,fontVariantNumeric:'tabular-nums',paddingLeft:0,paddingRight:0,borderWidth:0";
    let module = crate::session::tests::source(&format!(
        "function render(){{return h('view',{{style:{{alignItems:'flex-start'}}}},\
         h('text',{{'data-testid':'value',style:{{{digits},whiteSpace:'nowrap'}}}},'0000000000'),\
         h('input',{{'data-testid':'width',value:'2026-11-13',style:{{{digits},width:'10ch'}}}}),\
         h('view',{{style:{{width:40,flexDirection:'row'}}}},\
           h('input',{{'data-testid':'min',value:'2026-11-13',style:{{{digits},flexGrow:1,flexBasis:0,minWidth:'10ch'}}}})),\
         h('input',{{'data-testid':'max',value:'2026-11-13',style:{{{digits},width:300,maxWidth:'4ch'}}}}))}}"
    ));
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(320, 600, 1.0).unwrap();
    let width = |id: &str| {
        display
            .nodes
            .iter()
            .find(|node| node.test_id == id)
            .unwrap_or_else(|| panic!("{id}"))
            .bounds[2]
    };
    let ten = width("width");
    assert!(
        (ten - width("value")).abs() <= 1.0,
        "10ch is ten tabular digits (pixel-rounded): {ten} vs {}",
        width("value")
    );
    assert!(
        (width("min") - ten).abs() < 0.5,
        "min-width holds 10ch in a 40 pt row"
    );
    assert!(
        (width("max") - ten * 0.4).abs() < 0.5,
        "max-width caps at 4ch"
    );
}
