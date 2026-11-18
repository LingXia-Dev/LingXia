//! `<webview>` layout, identity stability, semantics, hit-testing, and `WebHole` composition tests.
use crate::ViewSession;
use crate::interaction::frame::PresentedFrame;
use crate::semantics::ids::SemanticIds;
use crate::semantics::projection::{Projection, Role};
use crate::session::lifecycle::CommitTicket;
use crate::session::tests::source;

#[test]
fn webview_requires_aria_label_and_projects_semantics_and_replaced_layout() {
    let missing = source("function render(){return h('webview',{src:'https://example.com'})}");
    let mut broken = ViewSession::new(&missing).unwrap();
    let Err(err) = broken.layout(300, 200, 1.0) else {
        panic!("expected UI_WEBVIEW error");
    };
    assert!(err.contains("UI_WEBVIEW"), "unexpected error: {err}");

    let module = source(
        "globalThis.src='https://example.com/a';globalThis.sheet=false;globalThis.show=true;\
         function render(){const web=useWebView('doc');return h('page',{},globalThis.show&&h('webview',{web,'aria-label':'Article','data-testid':'wv',src:globalThis.src}),globalThis.show&&h('dialog',{open:globalThis.sheet,'aria-label':'Sheet','data-testid':'sheet',style:{width:'100%',height:'70%',marginTop:'auto',marginBottom:0,marginLeft:0,marginRight:0}},h('webview',{web,'aria-label':'Article','data-testid':'wv-sheet',style:{width:'100%',height:'100%'}})))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let first = page.layout(300, 400, 1.0).unwrap();
    let node = first.nodes.iter().find(|n| n.test_id == "wv").unwrap();
    // Default replaced sizing: width 100% (300), height 150.
    assert_eq!(node.bounds, [0.0, 0.0, 300.0, 150.0]);
    let wv1 = node.webview.as_ref().unwrap().clone();
    assert_eq!(wv1.web_id, "wv-1");
    assert_eq!(wv1.src, "https://example.com/a");
    assert!(wv1.active);
    assert!(!wv1.in_top_layer);
    assert!(!wv1.degraded);
    assert!(!first.top_layer_open());

    // Semantics projection exposes Role::WebView with its accessible name.
    let presented = PresentedFrame::new(CommitTicket::new(1), 1.0, first.nodes.clone());
    let mut ids = SemanticIds::default();
    ids.update(presented.nodes.iter().map(|n| n.identity.as_ref()));
    let projection = Projection {
        frame: &presented,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
        drags: None,
        obscured: [0.0; 4],
        touch_target: 0.0,
    };
    let sem_id = projection
        .reading_order()
        .into_iter()
        .find(|id| projection.node(*id).is_some_and(|n| n.test_id == "wv"))
        .expect("webview semantic node");
    let sem = projection.node(sem_id).unwrap();
    assert_eq!(sem.role, Role::WebView);
    assert_eq!(sem.name, "Article");
    assert!(sem.focusable);

    // Opening a 70%-height `<dialog>` sheet hands the exact same `wv-1` session to the dialog slot!
    page.eval("globalThis.src='https://example.com/b';globalThis.sheet=true")
        .unwrap();
    let sheet = page.layout(300, 400, 1.0).unwrap();
    assert!(sheet.top_layer_open(), "dialog sheet lifts top layer");
    let inline_node = sheet.nodes.iter().find(|n| n.test_id == "wv").unwrap();
    assert!(!inline_node.webview.as_ref().unwrap().active);
    assert!(crate::scene::webview::mount_of(inline_node).is_none());

    let sheet_node = sheet
        .nodes
        .iter()
        .find(|n| n.test_id == "wv-sheet")
        .unwrap();
    assert_eq!(sheet_node.bounds, [0.0, 120.0, 300.0, 280.0]);
    let wv_sheet = sheet_node.webview.as_ref().unwrap();
    assert_eq!(wv_sheet.web_id, "wv-1");
    assert_eq!(wv_sheet.src, "https://example.com/b");
    assert!(wv_sheet.active);
    assert!(wv_sheet.in_top_layer);

    // Unmounting all `<webview>` nodes of the session and later remounting allocates a fresh `wv-2` id.
    page.eval("globalThis.show=false").unwrap();
    let empty = page.layout(300, 400, 1.0).unwrap();
    assert!(empty.nodes.iter().all(|n| n.webview.is_none()));
    page.eval("globalThis.show=true;globalThis.sheet=false")
        .unwrap();
    let remounted = page.layout(300, 400, 1.0).unwrap();
    let wv2 = remounted
        .nodes
        .iter()
        .find(|n| n.test_id == "wv")
        .unwrap()
        .webview
        .as_ref()
        .unwrap();
    assert_eq!(wv2.web_id, "wv-2");
}

#[test]
fn webview_at_respects_rounded_corners_scroll_clips_overlays_and_dialogs() {
    let module = source(
        "globalThis.modal=false;globalThis.faded=false;\
         function render(){return h('page',{},h('button',{'data-testid':'under',onClick(){},style:{position:'absolute',top:20,left:20,width:100,height:100}},'Under'),h('scroll',{style:{width:200,height:120,overflow:'hidden'}},h('view',{style:{opacity:globalThis.faded?0.5:1}},h('webview',{'aria-label':'Embed','data-testid':'wv',src:'https://example.com',style:{width:160,height:200,borderRadius:30}}))),h('button',{'data-testid':'fab',onClick(){},style:{position:'absolute',top:50,left:50,width:24,height:24}},'FAB'),globalThis.modal&&h('dialog',{open:true,'aria-label':'Confirm',style:{width:180,height:100}},h('button',{onClick(){}},'OK')))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    let frame = PresentedFrame::new(CommitTicket::new(1), 1.0, display.nodes.clone());

    // Inside the visible, non-covered interior of `<webview>` (and over `under` button, which is behind `<webview>`!):
    let hit = frame.webview_at([120.0, 80.0], 48.0);
    assert_eq!(hit.map(|(_, n)| n.test_id.as_str()), Some("wv"));
    // `under` button behind `<webview>` must NOT be hit through `<webview>`!
    assert_eq!(frame.press_hit([30.0, 80.0], 48.0), None);

    // Rounded corner exterior `(2, 2)` of `borderRadius: 30` is outside `<webview>`:
    assert!(frame.webview_at([2.0, 2.0], 48.0).is_none());

    // Below `<scroll>`'s 120px height (`y = 150`) is clipped away:
    assert!(frame.webview_at([80.0, 150.0], 48.0).is_none());

    // Directly on FAB `(62, 62)` AND inside FAB's 48px touch band `(42, 62)` -> FAB claims it, NOT `<webview>`!
    assert!(frame.webview_at([62.0, 62.0], 48.0).is_none());
    assert!(frame.webview_at([42.0, 62.0], 48.0).is_none());

    // Degraded (`opacity: 0.5` ancestor) disables live `<webview>` hit-testing:
    page.eval("globalThis.faded=true").unwrap();
    let faded_display = page.layout(300, 300, 1.0).unwrap();
    let faded_wv = faded_display
        .nodes
        .iter()
        .find(|n| n.test_id == "wv")
        .unwrap()
        .webview
        .as_ref()
        .unwrap();
    assert!(faded_wv.degraded);
    let faded_frame = PresentedFrame::new(CommitTicket::new(2), 1.0, faded_display.nodes);
    assert!(faded_frame.webview_at([120.0, 80.0], 48.0).is_none());

    // Open modal `<dialog>` blocks `<webview>` hit-testing and marks mount `accessibility_hidden`:
    page.eval("globalThis.faded=false;globalThis.modal=true")
        .unwrap();
    let modal_display = page.layout(300, 300, 1.0).unwrap();
    let modal_node = modal_display
        .nodes
        .iter()
        .find(|n| n.test_id == "wv")
        .unwrap();
    let mount = crate::scene::webview::mount_of(modal_node).unwrap();
    assert!(mount.accessibility_hidden);
    let modal_frame = PresentedFrame::new(CommitTicket::new(3), 1.0, modal_display.nodes);
    assert!(modal_frame.webview_at([120.0, 80.0], 48.0).is_none());
}

#[cfg(not(target_os = "android"))]
#[test]
fn webview_paints_surface_placeholder_until_ready_then_punches_dest_out_hole_preserving_overlays() {
    let module = source(
        "globalThis.faded=false;\
         function render(){return h('page',{style:{backgroundColor:'#ffffff'}},h('view',{style:{opacity:globalThis.faded?0.5:1}},h('webview',{'aria-label':'Doc','data-testid':'wv',src:'https://example.com',style:{width:100,height:100,borderRadius:24}})),h('button',{'aria-label':'Over',onClick(){},style:{position:'absolute',top:40,left:40,width:20,height:20,backgroundColor:'#ff0000'}}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let unready = page.layout(120, 120, 1.0).unwrap();
    let wv = unready
        .nodes
        .iter()
        .find(|n| n.test_id == "wv")
        .unwrap()
        .webview
        .as_ref()
        .unwrap()
        .clone();

    let pixel = |frame: &crate::Frame, x: usize, y: usize| -> [u8; 4] {
        let data = frame.pixels.data_as_u8_slice();
        let offset = (y * usize::from(frame.pixels.width()) + x) * 4;
        [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]
    };

    // 1. Before `set_ready`: `<webview>` interior `(20, 20)` is opaque `surface` placeholder (alpha == 255, NO black/transparent flash).
    let shot_unready = unready.rasterize().unwrap();
    assert_eq!(pixel(&shot_unready, 20, 20)[3], 255);

    // 2. Flip `set_ready(true)`: interior `(20, 20)` punches transparent hole `[0, 0, 0, 0]`,
    //    rounded corner exterior `(1, 1)` stays opaque white canvas `[255, 255, 255, 255]`,
    //    and red `<button>` painted above `<webview>` at `(50, 50)` stays opaque red `[255, 0, 0, 255]`!
    assert!(crate::scene::webview::set_ready(wv.scope, &wv.web_id, true));
    let ready = page.layout(120, 120, 1.0).unwrap();
    let shot_ready = ready.rasterize().unwrap();
    assert_eq!(pixel(&shot_ready, 20, 20), [0, 0, 0, 0]);
    assert_eq!(pixel(&shot_ready, 1, 1), [255, 255, 255, 255]);
    assert_eq!(pixel(&shot_ready, 50, 50), [255, 0, 0, 255]);

    // 3. When degraded (`opacity: 0.5`), hole-punch is suppressed so opaque blended surface shows.
    page.eval("globalThis.faded=true").unwrap();
    let degraded = page.layout(120, 120, 1.0).unwrap();
    let shot_degraded = degraded.rasterize().unwrap();
    assert_eq!(pixel(&shot_degraded, 20, 20)[3], 255);
}
