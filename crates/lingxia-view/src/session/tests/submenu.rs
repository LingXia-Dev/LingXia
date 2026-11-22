//! A menu item that opens a submenu: the menu stays, a choice closes the chain, and the arrows
//! open and close the submenu.
use super::*;
use crate::interaction::view::Key;

/// A button opening a menu whose "Move to" item opens a submenu beside it in the tree.
fn page(platform: &Platform, worker: &Arc<ViewWorker>, dir: &str) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(&format!(
            "function render(){{const log=useLocal('log','');return h('page',{{dir:'{dir}'}},h('button',{{popoverTarget:'menu','data-testid':'more'}},'More'),h('text',{{'data-testid':'log'}},log.value||'-'),h('view',{{id:'menu',popover:'auto',role:'menu','aria-label':'Actions','data-testid':'menu',style:{{width:80}}}},h('button',{{role:'menuitem','data-testid':'rename'}},'Rename'),h('button',{{role:'menuitem',popoverTarget:'move','data-testid':'move-to'}},'Move to')),h('view',{{id:'move',popover:'auto',role:'menu','aria-label':'Move to','data-testid':'move',style:{{width:80}}}},h('button',{{role:'menuitem','data-testid':'inbox',onClick(){{log.set(log.value+'inbox ')}}}},'Inbox'),h('button',{{role:'menuitem','data-testid':'archive'}},'Archive')))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

fn query(host: &ViewHost<Fake>, id: &str) -> serde_json::Value {
    host.automation_request(
        &serde_json::json!({"op": "query", "css": format!("[data-testid=\"{id}\"]")}),
    )
    .unwrap()
}

fn exists(host: &ViewHost<Fake>, id: &str) -> bool {
    query(host, id)["exists"].as_bool().unwrap()
}

fn focused(host: &ViewHost<Fake>, id: &str) -> bool {
    query(host, id)["focused"].as_bool().unwrap_or(false)
}

fn key(platform: &Platform, host: &ViewHost<Fake>, key: Key) -> bool {
    let taken = host.key(key, false, false);
    platform.settle(&[host]);
    run_frames(platform, host, 2);
    taken
}

/// Focus the invoker and open the menu from the keyboard, then move to "Move to".
fn open_menu(platform: &Platform, host: &ViewHost<Fake>) {
    key(platform, host, Key::Tab);
    assert!(focused(host, "more"));
    key(platform, host, Key::Activate);
    assert!(
        focused(host, "rename"),
        "the menu takes focus on its first item"
    );
    key(platform, host, Key::Down);
    assert!(focused(host, "move-to"));
}

#[test]
fn the_forward_arrow_opens_a_submenu_and_the_back_arrow_or_escape_closes_only_it() {
    let worker = ViewWorker::spawn("test-submenu-keys").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "ltr");
    open_menu(&platform, &host);
    assert!(
        !key(&platform, &host, Key::Left) || exists(&host, "menu"),
        "back in a top menu closes nothing"
    );
    assert!(exists(&host, "menu"));
    assert!(key(&platform, &host, Key::Right));
    assert!(
        exists(&host, "menu") && exists(&host, "move"),
        "the menu stays under its submenu"
    );
    assert!(
        focused(&host, "inbox"),
        "the submenu takes focus on its first item"
    );
    assert!(key(&platform, &host, Key::Left));
    assert!(exists(&host, "menu") && !exists(&host, "move"));
    assert!(
        focused(&host, "move-to"),
        "focus returns to the item that opened it"
    );
    key(&platform, &host, Key::Right);
    key(&platform, &host, Key::Down);
    assert!(focused(&host, "archive"), "arrows stay inside the submenu");
    assert!(key(&platform, &host, Key::Escape));
    assert!(
        exists(&host, "menu") && !exists(&host, "move"),
        "Escape closes the innermost"
    );
    assert!(focused(&host, "move-to"));
    assert!(key(&platform, &host, Key::Escape));
    assert!(!exists(&host, "menu"));
    assert!(focused(&host, "more"));
}

#[test]
fn a_choice_in_a_submenu_closes_the_whole_chain() {
    let worker = ViewWorker::spawn("test-submenu-choice").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "ltr");
    open_menu(&platform, &host);
    key(&platform, &host, Key::Right);
    assert!(focused(&host, "inbox"));
    key(&platform, &host, Key::Activate);
    assert_eq!(query(&host, "log")["text"], "inbox ");
    assert!(!exists(&host, "menu") && !exists(&host, "move"));
    assert!(
        focused(&host, "more"),
        "focus returns to what opened the menu"
    );
}

#[test]
fn a_press_on_another_item_reaches_it_past_the_open_submenu() {
    let worker = ViewWorker::spawn("test-submenu-press").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "ltr");
    open_menu(&platform, &host);
    key(&platform, &host, Key::Right);
    assert!(exists(&host, "move"));
    let node = query(&host, "rename");
    let at = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        host.pointer(Pointer {
            phase,
            x: (at("left") + at("width") / 2.0) * 2.0,
            y: (at("top") + at("height") / 2.0) * 2.0,
            slop: 8.0,
            velocity: [0.0; 2],
            min_fling: 50.0,
        });
    }
    platform.settle(&[&host]);
    run_frames(&platform, &host, 2);
    assert!(
        !exists(&host, "menu") && !exists(&host, "move"),
        "one press chose Rename"
    );
}

#[test]
fn a_submenu_a_press_opened_hands_focus_back_to_its_item() {
    let worker = ViewWorker::spawn("test-submenu-press-open").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "ltr");
    let click = |id: &str| {
        host.automation_request(&serde_json::json!({"op": "act", "css": format!("[data-testid=\"{id}\"]"), "action": {"kind": "click"}}))
            .unwrap();
        platform.settle(&[&host]);
        run_frames(&platform, &host, 2);
    };
    click("more");
    click("move-to");
    assert!(exists(&host, "move"));
    assert!(key(&platform, &host, Key::Left));
    assert!(exists(&host, "menu") && !exists(&host, "move"));
    assert!(focused(&host, "move-to"), "not the menu's first item");
    assert!(key(&platform, &host, Key::Escape));
    assert!(!exists(&host, "menu"));
    assert!(focused(&host, "more"));
}

#[test]
fn the_arrows_swap_in_a_right_to_left_menu() {
    let worker = ViewWorker::spawn("test-submenu-rtl").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker, "rtl");
    open_menu(&platform, &host);
    key(&platform, &host, Key::Right);
    assert!(!exists(&host, "move"), "Right points back");
    assert!(key(&platform, &host, Key::Left));
    assert!(exists(&host, "move") && focused(&host, "inbox"));
    assert!(key(&platform, &host, Key::Right));
    assert!(exists(&host, "menu") && !exists(&host, "move"));
    assert!(focused(&host, "move-to"));
}

/// The same menu with its submenu asked to sit at the inline end of its item.
fn side_page(platform: &Platform, worker: &Arc<ViewWorker>, width: u32) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(&format!(
            "function render(){{return h('page',{{}},h('button',{{popoverTarget:'menu','data-testid':'more'}},'More'),h('view',{{id:'menu',popover:'auto',role:'menu','aria-label':'Actions','data-testid':'menu',style:{{width:{width}}}}},h('button',{{role:'menuitem','data-testid':'rename'}},'A'),h('button',{{role:'menuitem',popoverTarget:'move','data-testid':'move-to'}},'B')),h('view',{{id:'move',popover:'auto',role:'menu','aria-label':'Move to','data-testid':'move',style:{{width:{width},positionArea:'inline-end span-block-end'}}}},h('button',{{role:'menuitem','data-testid':'inbox'}},'C')))}}"
        )),
        false,
    );
    host.set_visible(true);
    platform.settle(&[&host]);
    host
}

/// Left, top, right and bottom of the element with test id `id`.
fn edges(host: &ViewHost<Fake>, id: &str) -> [f32; 4] {
    let node = query(host, id);
    let at = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    [
        at("left"),
        at("top"),
        at("left") + at("width"),
        at("top") + at("height"),
    ]
}

#[test]
fn a_submenu_sits_beside_its_item_and_under_it_when_neither_side_has_room() {
    let worker = ViewWorker::spawn("test-submenu-side").unwrap();
    let platform = Platform::new(&[&worker]);
    let narrow = side_page(&platform, &worker, 40);
    open_menu(&platform, &narrow);
    key(&platform, &narrow, Key::Right);
    let (menu, item, sub) = (
        edges(&narrow, "menu"),
        edges(&narrow, "move-to"),
        edges(&narrow, "move"),
    );
    assert!(
        sub[0] >= menu[2],
        "beside the menu: {sub:?} against {menu:?}"
    );
    assert_eq!(sub[1], item[1], "level with its item");

    let wide = side_page(&platform, &worker, 80);
    open_menu(&platform, &wide);
    key(&platform, &wide, Key::Right);
    let (item, sub) = (edges(&wide, "move-to"), edges(&wide, "move"));
    assert!(
        sub[1] >= item[3],
        "under its item, which stays in view: {sub:?} against {item:?}"
    );
    assert!(sub[0] >= 0.0 && sub[2] <= 100.0, "on screen: {sub:?}");
    assert!(focused(&wide, "inbox"));
}
