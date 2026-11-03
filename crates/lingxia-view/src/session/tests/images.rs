use crate::ViewSession;
#[test]
fn image_callback_errors_do_not_consume_other_consumers_or_their_updates() {
    let module = crate::session::tests::source(
        "function render(){const status=useLocal('status','initial');return h('column',{},h('image',{src:'photo',onLoad:()=>{status.set('bad');throw Error('first')},style:{width:20,height:20}}),h('image',{src:'photo',onLoad:()=>status.set('ready'),style:{width:20,height:20}}),h('image',{src:'photo',onLoad:()=>{status.set('bad');throw Error('last')},style:{width:20,height:20}}),h('text',{'data-testid':'status'},status.value))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.layout(200, 200, 1.0).unwrap();
    page.image_completed(page.image_requests()[0].clone(), true);
    assert!(page.dispatch_image_events(page.generation()).unwrap());
    assert_eq!(page.take_action_errors().unwrap().len(), 2);
    let frame = page.layout(200, 200, 1.0).unwrap();
    assert_eq!(
        frame
            .nodes
            .iter()
            .find(|n| n.test_id == "status")
            .unwrap()
            .text,
        "ready"
    );
    assert!(!page.dispatch_image_events(page.generation()).unwrap());
}

#[test]
fn regenerated_image_callbacks_reuse_layout() {
    let module = crate::session::tests::source(
        "function render(){return h('image',{src:'photo',onLoad(){},onError(){},style:{width:20,height:20}})}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.layout(200, 200, 1.0).unwrap();
    page.layout(200, 200, 1.0).unwrap();
    assert_eq!(page.frame_metrics().layout_passes, 0);
}

#[test]
fn image_outcomes_follow_committed_identity_and_retry_without_idle_loops() {
    let module = crate::session::tests::source(
        "globalThis.retry='';globalThis.extra=false;function render(){const status=useLocal('status','loading');return h('column',{},h('image',{key:'photo',src:'photo',retryKey:globalThis.retry,onLoad:()=>status.set('ready'),onError:()=>status.set('failed'),style:{width:20,height:20}}),globalThis.extra?h('image',{key:'extra',src:'photo',retryKey:globalThis.retry,onLoad:()=>status.set('cached'),style:{width:20,height:20}}):null,h('text',{'data-testid':'status'},status.value))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    page.layout(200, 200, 1.0).unwrap();
    let original = page.image_requests()[0].clone();
    page.image_completed(original.clone(), false);
    assert!(
        !page
            .dispatch_image_events(crate::session::lifecycle::CommitTicket::new(
                page.generation().get() - 1
            ))
            .unwrap()
    );
    assert!(page.dispatch_image_events(page.generation()).unwrap());
    let frame = page.layout(200, 200, 1.0).unwrap();
    assert_eq!(
        frame
            .nodes
            .iter()
            .find(|n| n.test_id == "status")
            .unwrap()
            .text,
        "failed"
    );
    assert!(!page.dispatch_image_events(page.generation()).unwrap());
    page.eval("globalThis.retry='1'").unwrap();
    page.layout(200, 200, 1.0).unwrap();
    page.image_completed(original, true);
    assert!(!page.dispatch_image_events(page.generation()).unwrap());
    page.image_completed(page.image_requests()[0].clone(), true);
    assert!(page.dispatch_image_events(page.generation()).unwrap());
    page.layout(200, 200, 1.0).unwrap();
    page.eval("globalThis.extra=true").unwrap();
    page.layout(200, 200, 1.0).unwrap();
    assert!(page.dispatch_image_events(page.generation()).unwrap());
    let frame = page.layout(200, 200, 1.0).unwrap();
    assert_eq!(
        frame
            .nodes
            .iter()
            .find(|n| n.test_id == "status")
            .unwrap()
            .text,
        "cached"
    );
    assert!(!page.dispatch_image_events(page.generation()).unwrap());
}
