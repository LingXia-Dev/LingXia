//! A `<List>` named with `aria-label` says that name to assistive tech.
use super::*;

#[test]
fn a_named_list_says_its_name_to_assistive_tech() {
    let worker = ViewWorker::spawn("test-list-name").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source(
            "function render(){return h('page',{},h(listTag,{count:3,height:()=>20,'aria-label':'Messages','data-testid':'thread',style:{height:60},row:i=>h('text',{},'row '+i)}))}",
        ),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let list = host
        .automation_request(&serde_json::json!({"op": "query", "css": "[data-testid=\"thread\"]"}))
        .unwrap();
    assert_eq!(list["aria_label"], "Messages", "{list}");
    // It stays the scrolling container it is to assistive tech, now with a name.
    let named = host.semantics(|projection, _| {
        let projection = projection.unwrap();
        projection
            .frame
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, _)| projection.ids.id(index).and_then(|id| projection.node(id)))
            .any(|node| {
                node.role == crate::semantics::projection::Role::ScrollView
                    && node.name == "Messages"
            })
    });
    assert!(named);
}
