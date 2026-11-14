//! A vector or image keeps its own size unless it asks to stretch across its line.

fn bounds(body: &str, id: &str) -> [f32; 4] {
    let module = crate::session::tests::source(&format!("function render(){{return {body}}}"));
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(300, 800, 1.0).unwrap();
    display
        .nodes
        .iter()
        .find(|node| node.test_id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .bounds
}

const VECTOR: &str =
    "viewBox:[0,0,100,100],paths:[{d:'M0 0L100 100',stroke:'#000000',fill:'none'}]";

fn vector(parent: &str, style: &str) -> [f32; 4] {
    bounds(
        &format!(
            "h('view',{{style:{{width:300,height:40,{parent}}}}},h('vector',{{'data-testid':'v',{VECTOR},style:{{{style}}}}}))"
        ),
        "v",
    )
}

#[test]
fn a_self_stretched_vector_fills_its_line_and_keeps_its_own_size_along_it() {
    // Across a column, as a chart's drawing in its plot.
    assert_eq!(vector("", "alignSelf:'stretch'")[2..], [300.0, 24.0]);
    assert_eq!(
        vector("", "alignSelf:'stretch',flexGrow:1")[2..],
        [300.0, 40.0]
    );
    // Down a row, as a sparkline beside a figure.
    assert_eq!(
        vector("flexDirection:'row'", "alignSelf:'stretch'")[2..],
        [24.0, 40.0]
    );
    assert_eq!(
        vector("flexDirection:'row'", "alignSelf:'stretch',flexGrow:1")[2..],
        [300.0, 40.0]
    );
}

#[test]
fn an_unstretched_vector_keeps_its_icon_size_in_a_stretching_parent() {
    assert_eq!(vector("", "")[2..], [24.0, 24.0]);
    assert_eq!(vector("alignItems:'stretch'", "")[2..], [24.0, 24.0]);
    assert_eq!(
        vector("flexDirection:'row',alignItems:'stretch'", "")[2..],
        [24.0, 24.0]
    );
    // Main-axis growth already reaches it.
    assert_eq!(vector("", "flexGrow:1")[2..], [24.0, 40.0]);
}

#[test]
fn a_self_stretched_image_fills_its_line() {
    let image = |style: &str| {
        bounds(
            &format!(
                "h('view',{{style:{{width:300,height:80}}}},h('image',{{'data-testid':'i',src:'a.png',style:{{{style}}}}}))"
            ),
            "i",
        )
    };
    assert_eq!(image("")[2..], [64.0, 64.0]);
    assert_eq!(image("alignSelf:'stretch'")[2..], [300.0, 64.0]);
}
