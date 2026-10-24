use crate::{ViewSession, schema};
use serde_json::json;

#[test]
fn flex_container_directions_reflow_without_changing_identity() {
    let source = crate::session::tests::source(
        "globalThis.direction='row';function render(){return h('view',{style:{flexDirection:direction,display:'flex',gap:10}},[0,1].map(i=>h('column',{key:String(i),testId:'item'+i,style:{width:40,height:20}})))}",
    );
    let mut page = ViewSession::new(&source).unwrap();
    let mut identities = None;
    for (direction, first, second) in [
        ("row", [0.0, 0.0], [50.0, 0.0]),
        ("column", [0.0, 0.0], [0.0, 30.0]),
        ("row-reverse", [120.0, 0.0], [70.0, 0.0]),
        ("column-reverse", [0.0, 140.0], [0.0, 110.0]),
    ] {
        page.eval(&format!("direction='{direction}'")).unwrap();
        let display = page.layout(160, 160, 1.0).unwrap();
        for (id, position) in [("item0", first), ("item1", second)] {
            let node = display
                .nodes
                .iter()
                .find(|node| node.test_id == id)
                .unwrap();
            assert_eq!(
                node.bounds,
                [position[0], position[1], 40.0, 20.0],
                "{direction}"
            );
        }
        let current: Vec<_> = display
            .nodes
            .iter()
            .map(|node| node.identity.clone())
            .collect();
        if let Some(previous) = &identities {
            assert_eq!(previous, &current);
        }
        identities = Some(current);
        assert!(display.rasterize().is_ok());
    }
}

#[test]
fn wire_keeps_flex_layout_options_bounded_and_container_specific() {
    assert_eq!(schema::FIELDS[103], ("flexDirection", "flex-direction"));
    assert_eq!(schema::FIELDS[104], ("display", "flex-display"));
    for tag in [2, 3, 5, 6, 16, 17] {
        for (field, value) in [(103, "row"), (104, "flex")] {
            let wire = json!([tag, -1, "", "", "", 1, field, value]);
            assert!(schema::decode(wire.as_array().unwrap()).is_err());
        }
    }
    for (field, value) in [(103, "sideways"), (104, "grid"), (104, "none")] {
        assert!(
            schema::decode(
                json!([0, -1, "", "", "", 1, field, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
}
