use super::*;

/// A page under a mouse (a popover leaves the page live) or a finger, with motion reduced.
fn page(
    platform: &Platform,
    worker: &Arc<ViewWorker>,
    pointer: &str,
    render: &str,
) -> ViewHost<Fake> {
    let host = platform.host(worker, &source(render), false);
    if let Some(environment) = host.adapter().environment.lock().unwrap().as_mut() {
        environment.value.pointer = crate::protocol::env::Pointer::parse(pointer).unwrap();
        environment.value.reduced_motion = Some(true);
    }
    host.set_visible(true);
    platform.settle(&[&host]);
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
}

#[test]
fn a_picker_closes_when_its_editor_unmounts() {
    for pointer in ["fine", "coarse"] {
        for kind in ["date", "select"] {
            let worker = ViewWorker::spawn("test-orphan-picker").unwrap();
            let platform = Platform::new(&[&worker]);
            let editor = if kind == "date" {
                "h('input',{'data-testid':'field',type:'date',bind:day,'aria-label':'Day'})"
            } else {
                "h('select',{'data-testid':'field',bind:day,'aria-label':'Plan',options:[{value:'a',label:'A'},{value:'b',label:'B'}]})"
            };
            // The content switches away, as a workspace pane changing pages.
            let host = page(
                &platform,
                &worker,
                pointer,
                &format!(
                    "function render(){{const shown=useLocal('shown',true),day=useLocal('day','2026-10-21');return h('page',{{}},h('button',{{'data-testid':'away',onClick(){{shown.set(false)}}}},'Away'),shown.value?{editor}:h('text',{{'data-testid':'other'}},'Other page'))}}"
                ),
            );
            click(&platform, &host, "field");
            assert_eq!(count(&host, "lx-picker"), 1, "{pointer} {kind}: opened");
            click(&platform, &host, "away");
            assert_eq!(
                count(&host, "other"),
                1,
                "{pointer} {kind}: the editor is gone"
            );
            assert_eq!(
                count(&host, "lx-picker"),
                0,
                "{pointer} {kind}: its picker went with it"
            );
        }
    }
}

/// A popover `tip` its `trigger` opens, holding the button that unmounts the trigger; `log`
/// spells its toggles.
fn popover_page(kind: &str, controlled: bool) -> String {
    let open = if controlled { "open:open.value," } else { "" };
    format!(
        "function render(){{const shown=useLocal('shown',true),open=useLocal('open',false),log=useLocal('log','');return h('page',{{}},shown.value?h('button',{{popoverTarget:'tip','data-testid':'trigger'}},'Info'):h('text',{{'data-testid':'other'}},'Other'),h('view',{{id:'tip',popover:'{kind}',{open}onToggle(next){{open.set(next);log.set(log.value+(next?'o':'c'))}},'data-testid':'tip'}},h('button',{{'data-testid':'away',onClick(){{shown.set(false)}}}},'Away')),h('text',{{'data-testid':'log-'+log.value}},log.value))}}"
    )
}

#[test]
fn a_popover_closes_when_the_invoker_that_opened_it_unmounts() {
    for kind in ["auto", "hint", "manual"] {
        for controlled in [false, true] {
            let worker = ViewWorker::spawn("test-orphan-popover").unwrap();
            let platform = Platform::new(&[&worker]);
            let host = page(&platform, &worker, "fine", &popover_page(kind, controlled));
            click(&platform, &host, "trigger");
            assert_eq!(count(&host, "tip"), 1, "{kind} {controlled}: opened");
            click(&platform, &host, "away");
            assert_eq!(
                count(&host, "other"),
                1,
                "{kind} {controlled}: the invoker is gone"
            );
            assert_eq!(
                count(&host, "tip"),
                0,
                "{kind} {controlled}: its popover went with it"
            );
            assert_eq!(count(&host, "log-oc"), 1, "{kind} {controlled}: told once");
        }
    }
}

#[test]
fn a_hint_closes_when_the_interest_trigger_showing_it_unmounts() {
    let worker = ViewWorker::spawn("test-orphan-hint").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "fine",
        "function render(){const shown=useLocal('shown',true);return h('page',{},shown.value?h('button',{interestFor:'tip','aria-label':'About','data-testid':'about',style:{width:100,height:40}}):h('text',{'data-testid':'other'},'Other'),h('view',{id:'tip',popover:'hint','data-testid':'tip'},h('button',{'data-testid':'away',onClick(){shown.set(false)}},'Away')))}",
    );
    host.hover(Some([20.0, 20.0]));
    platform.advance(Duration::from_millis(500));
    platform.settle(&[&host]);
    run_frames(&platform, &host, 4);
    assert_eq!(count(&host, "tip"), 1, "the hint shows after the rest");
    click(&platform, &host, "away");
    assert_eq!(count(&host, "other"), 1);
    assert_eq!(count(&host, "tip"), 0, "no grace: its trigger is gone");
}

#[test]
fn a_popover_opened_from_code_or_whose_invoker_only_renders_again_stays() {
    let worker = ViewWorker::spawn("test-orphan-not").unwrap();
    let platform = Platform::new(&[&worker]);
    // Opened by `open`, not by an invoker: the button that set it may go.
    let host = page(
        &platform,
        &worker,
        "fine",
        "function render(){const shown=useLocal('shown',true),open=useLocal('open',false);return h('page',{},shown.value?h('button',{'data-testid':'opener',onClick(){open.set(true)}},'Open'):h('text',{'data-testid':'other'},'Other'),h('view',{id:'tip',popover:'manual',open:open.value,'data-testid':'tip'},h('button',{'data-testid':'away',onClick(){shown.set(false)}},'Away')))}",
    );
    click(&platform, &host, "opener");
    click(&platform, &host, "away");
    assert_eq!(count(&host, "other"), 1);
    assert_eq!(count(&host, "tip"), 1, "nothing owns it");

    // The invoker renders again in place and stays the owner; under a new key it is another control.
    let host = page(
        &platform,
        &worker,
        "fine",
        "function render(){const n=useLocal('n',0),k=useLocal('k',0);return h('page',{},h('button',{key:'k'+k.value,popoverTarget:'tip','data-testid':'trigger'},'Info '+n.value),h('view',{id:'tip',popover:'manual','data-testid':'tip'},h('button',{'data-testid':'again',onClick(){n.set(n.value+1)}},'Again'),h('button',{'data-testid':'rekey',onClick(){k.set(k.value+1)}},'Rekey')))}",
    );
    click(&platform, &host, "trigger");
    click(&platform, &host, "again");
    assert_eq!(count(&host, "tip"), 1, "the same invoker, rendered again");
    click(&platform, &host, "rekey");
    assert_eq!(count(&host, "tip"), 0, "a new control took its place");
}

#[test]
fn a_popover_closes_when_its_invoker_row_leaves_the_list() {
    let worker = ViewWorker::spawn("test-orphan-row").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(
        &platform,
        &worker,
        "fine",
        "function render(){const gone=useLocal('gone',false);const rows=Array.from({length:6},(_,i)=>({id:`row-${i}`})).filter(r=>!(gone.value&&r.id==='row-1'));return h('view',{},h(List,{'data-testid':'list',style:{height:240},items:rows,getKey:r=>r.id,renderItem:r=>h('button',{'data-testid':r.id,popoverTarget:'menu',style:{height:40}},r.id)}),h('view',{id:'menu',popover:'auto','data-testid':'menu'},h('button',{'data-testid':'delete',onClick(){gone.set(true)}},'Delete')))}",
    );
    click(&platform, &host, "row-1");
    assert_eq!(count(&host, "menu"), 1, "opened from the row");
    click(&platform, &host, "delete");
    assert_eq!(count(&host, "row-1"), 0, "the row is gone");
    assert_eq!(count(&host, "menu"), 0, "its menu went with it");
}

#[test]
fn a_press_outside_an_open_picker_closes_it_and_presses_nothing() {
    for kind in ["date", "select"] {
        let worker = ViewWorker::spawn("test-picker-outside").unwrap();
        let platform = Platform::new(&[&worker]);
        let editor = if kind == "date" {
            "h('input',{'data-testid':'field',type:'date',bind:day,'aria-label':'Day'})"
        } else {
            "h('select',{'data-testid':'field',bind:day,'aria-label':'Plan',options:[{value:'a',label:'A'},{value:'b',label:'B'}]})"
        };
        let host = page(
            &platform,
            &worker,
            "fine",
            &format!(
                "function render(){{const day=useLocal('day','2026-10-21'),hit=useLocal('hit',0);return h('page',{{}},{editor},h('button',{{'data-testid':'under-'+hit.value,onClick(){{hit.set(hit.value+1)}},style:{{position:'absolute',left:0,bottom:0,width:40,height:20}}}},'U'))}}"
            ),
        );
        click(&platform, &host, "field");
        assert_eq!(count(&host, "lx-picker"), 1, "{kind}: opened");
        // The fake display is 200 x 400; the button sits in its bottom-left corner.
        tap(&host, 10.0, 395.0);
        platform.settle(&[&host]);
        assert_eq!(
            count(&host, "lx-picker"),
            0,
            "{kind}: closed by the press outside"
        );
        assert_eq!(
            count(&host, "under-0"),
            1,
            "{kind}: the press reached nothing under it"
        );
    }
}
