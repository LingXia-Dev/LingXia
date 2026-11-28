//! `onLayout` and `useBox`: an element's content box reaches the page one frame after the layout
//! that changed it.
use super::*;

fn page(platform: &Platform, worker: &Arc<ViewWorker>, render: &str) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(render), false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(platform, &host, 4);
    host
}

fn count(host: &ViewHost<Fake>, id: &str) -> u64 {
    host.automation_request(
        &serde_json::json!({"op":"query","css":format!("[data-testid=\"{id}\"]"),"all":true}),
    )
    .unwrap()["count"]
        .as_u64()
        .unwrap()
}

fn click(platform: &Platform, host: &ViewHost<Fake>, id: &str) {
    host.automation_request(&serde_json::json!({"op":"act","css":format!("[data-testid=\"{id}\"]"),"action":{"kind":"click"}}))
        .unwrap();
    platform.settle(&[host]);
    run_frames(platform, host, 4);
}

#[test]
fn a_box_reads_its_content_size_in_render_and_follows_each_change() {
    let worker = ViewWorker::spawn("test-box").unwrap();
    let platform = Platform::new(&[&worker]);
    // 120 x 50 with 10 of padding and a 1 pt border: a 98 x 28 content box. `log` spells every call.
    let host = page(
        &platform,
        &worker,
        "function render(){const box=useBox('m'),wide=useLocal('wide',false),n=useLocal('n',0),log=useLocal('log','');const b=box.value;return h('page',{},h('view',{box,onLayout(s){log.set(log.value+s.width+'x'+s.height+';')},style:{width:wide.value?200:120,height:50,padding:10,borderWidth:1,borderColor:'#000000'}}),h('button',{'data-testid':'widen',onClick(){wide.set(true)}},'W'),h('button',{'data-testid':'again',onClick(){n.set(n.value+1)}},'A'),h('text',{'data-testid':'box-'+(b?b.width+'x'+b.height:'none')},'b'),h('text',{'data-testid':'log-'+log.value},'l'),h('text',{'data-testid':'n-'+n.value},'n'))}",
    );
    assert_eq!(
        count(&host, "box-98x28"),
        1,
        "read in render after the first layout"
    );
    assert_eq!(count(&host, "log-98x28;"), 1, "told once");
    click(&platform, &host, "again");
    assert_eq!(count(&host, "n-1"), 1);
    assert_eq!(
        count(&host, "log-98x28;"),
        1,
        "a render that keeps the size tells nothing"
    );
    click(&platform, &host, "widen");
    assert_eq!(count(&host, "box-178x28"), 1, "the new size");
    assert_eq!(count(&host, "log-98x28;178x28;"), 1, "one call per change");
}

#[test]
fn a_box_costs_one_more_frame_and_then_settles() {
    let worker = ViewWorker::spawn("test-box-first").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = platform.host(
        &worker,
        &source("function render(){const box=useBox('m');const b=box.value;return h('page',{},h('view',{box,style:{width:33,height:20}}),h('text',{'data-testid':'box-'+(b?b.width+'x'+b.height:'none')},'b'))}"),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    let first = host.adapter().log.lock().unwrap().presented.len();
    assert!(first >= 1);
    run_frames(&platform, &host, 4);
    assert_eq!(count(&host, "box-33x20"), 1);
    let frames = host.adapter().log.lock().unwrap().presented.len();
    run_frames(&platform, &host, 4);
    assert_eq!(
        host.adapter().log.lock().unwrap().presented.len(),
        frames,
        "settled: the size it reported renders nothing more"
    );
}

#[test]
fn text_button_scroll_vector_and_plot_report_a_box_and_a_remounted_element_reports_again() {
    let worker = ViewWorker::spawn("test-box-tags").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "function render(){const seen=useLocal('seen',''),shown=useLocal('shown',true);const on=t=>s=>{if(s.width>0&&s.height>0)seen.set(seen.value+t)};return h('page',{},h('button',{'data-testid':'flip',onClick(){shown.set(!shown.value)}},'F'),shown.value&&h('text',{key:'t',onLayout:on('T')},'Text'),h('scroll',{onLayout:on('S'),style:{height:40}},h('view',{style:{height:200}})),h('vector',{onLayout:on('V'),paths:[{d:'M0 0L10 10',stroke:'#000000'}],style:{width:20,height:20}}),h('plot',{onLayout:on('P'),kind:'bar',values:[1,2],'aria-label':'P',style:{height:60}}),h('button',{onLayout:on('B'),onClick(){}},'Go'),h('text',{'data-testid':'seen-'+seen.value.split('').sort().join('')},'s'))}",
    );
    assert_eq!(count(&host, "seen-BPSTV"), 1, "each element reported once");
    click(&platform, &host, "flip");
    assert_eq!(count(&host, "seen-BPSTV"), 1, "unmounting reports nothing");
    click(&platform, &host, "flip");
    assert_eq!(
        count(&host, "seen-BPSTTV"),
        1,
        "mounted again, the text reports again"
    );
}

#[test]
fn a_box_reports_its_contents_min_and_max_content_widths_inside_its_padding() {
    // The same words padded and bare, and the longest word alone; the boxes are wider than
    // the line, so their own width says nothing about the content.
    let mut session = crate::ViewSession::new(&source(
        "function render(){const on=()=>{};const words=t=>h('text',{style:{fontSize:16}},t);return h('page',{},h('view',{onLayout:on,'data-testid':'padded',style:{width:300,padding:10,borderWidth:1,borderColor:'#000000'}},words('Hello wonderful world')),h('view',{onLayout:on,'data-testid':'bare',style:{width:300}},words('Hello wonderful world')),h('view',{onLayout:on,'data-testid':'word',style:{width:300}},words('wonderful')),h('view',{'data-testid':'unread',style:{width:300}},words('Hello')))}",
    ))
    .unwrap();
    let display = session.layout(400, 400, 1.0).unwrap();
    let widths = |id: &str| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .intrinsic
    };
    let [min, max] = widths("padded").unwrap();
    assert_eq!(
        widths("bare"),
        Some([min, max]),
        "padding and border excluded"
    );
    let [word, word_max] = widths("word").unwrap();
    assert_eq!(
        (min, word),
        (word_max, word_max),
        "narrowest: the longest word"
    );
    assert!(max > min * 2.0 && max < 278.0, "one line: {max}");
    assert_eq!(widths("unread"), None, "only a box a page reads is probed");
}

#[test]
fn a_box_delivers_its_content_widths_to_the_page() {
    let worker = ViewWorker::spawn("test-box-widths").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "function render(){const box=useBox('m');const b=box.value;return h('page',{},h('view',{box,style:{width:300}},h('text',{style:{fontSize:16}},'Hello wonderful world')),h('text',{'data-testid':'fits-'+(b?String(b.minContentWidth>0&&b.maxContentWidth>b.minContentWidth&&b.maxContentWidth<b.width):'none')},'f'))}",
    );
    assert_eq!(count(&host, "fits-true"), 1);
}

#[cfg(feature = "hot-swap")]
#[test]
fn a_hot_swap_measures_every_box_again() {
    let worker = ViewWorker::spawn("test-box-swap").unwrap();
    let platform = Platform::new(&[&worker]);
    let render = "function render(){const box=useBox('m'),log=useLocal('log','');const b=box.value;return h('page',{},h('view',{box,onLayout(s){log.set(log.value+s.width+';')},style:{width:40,height:20}}),h('text',{'data-testid':'box-'+(b?b.width:'none')},'b'),h('text',{'data-testid':'log-'+log.value},'l'))}";
    let module = source(render) + "\nenableHotSwap();";
    let host = platform.host(&worker, &module, false);
    host.set_visible(true);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert_eq!(count(&host, "box-40"), 1);
    assert_eq!(count(&host, "log-40;"), 1);
    // The new isolate's box starts empty and its `onLayout` has not heard the size.
    host.swap_module(module);
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert_eq!(
        count(&host, "box-40"),
        1,
        "the swapped module reads its box"
    );
    assert_eq!(count(&host, "log-40;40;"), 1, "and is told the size again");
}
