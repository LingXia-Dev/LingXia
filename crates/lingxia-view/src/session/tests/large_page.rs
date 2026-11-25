use super::*;

// A 4,000-node page laid out from cold, timed: the page's own flow-end scan must not run per node.
#[test]
#[ignore = "measurement: cargo test -p lingxia-view --lib large_page -- --ignored --nocapture"]
fn large_page_layout_cost() {
    let module = source(
        "function render(){return h('page',{},h('scroll',{},...Array.from({length:1000},(_,i)=>h('view',{key:String(i),style:{flexDirection:'row',height:40}},h('text',{},'Row '+i),h('view',{style:{width:8,height:8}}),h('text',{},String(i))))))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let started = std::time::Instant::now();
    page.layout(400, 800, 1.0).unwrap();
    println!("4k-node page, first layout: {:?}", started.elapsed());
}
