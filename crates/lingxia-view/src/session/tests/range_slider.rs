//! A slider with two values: two sliders over one track. A press goes to the nearer thumb, the
//! thumbs stop at each other, keys move the focused one, and assistive tech meets two sliders.
use super::*;
use crate::interaction::view::Key;
use crate::semantics::projection::Role;

/// A price range of 20 to 60 in steps of 10, with its values and its finished changes as text.
/// The test surface is 100 wide.
fn page(platform: &Platform, worker: &Arc<ViewWorker>) -> ViewHost<Fake> {
    let host = platform.host(
        worker,
        &source(
            "function render(){const r=useLocal('r',[20,60]);const log=useLocal('log','');return h('page',{},h('slider',{'data-testid':'price','aria-label':'Price',thumbLabels:['From','To'],values:r.value,step:10,style:{width:96,height:40},onChange(v){r.set(v)},onChangeEnd(v){log.set(log.value+v.join('-')+' ')}}),h('text',{'data-testid':'now'},r.value.join('-')),h('text',{'data-testid':'log'},log.value||'-'))}",
        ),
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

fn text(host: &ViewHost<Fake>, id: &str) -> String {
    query(host, id)["text"].as_str().unwrap().to_owned()
}

fn values(host: &ViewHost<Fake>) -> [u32; 2] {
    let now = text(host, "now");
    let (lower, upper) = now.split_once('-').unwrap();
    [lower.parse().unwrap(), upper.parse().unwrap()]
}

/// A pointer event `share` of the way along the slider's box.
fn pointer(platform: &Platform, host: &ViewHost<Fake>, phase: PointerPhase, share: f32) {
    let node = query(host, "price");
    let at = |key: &str| node["rect"][key].as_f64().unwrap() as f32;
    host.pointer(Pointer {
        phase,
        x: (at("left") + at("width") * share) * 2.0,
        y: (at("top") + at("height") / 2.0) * 2.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    });
    run_frames(platform, host, 6);
}

fn tap(platform: &Platform, host: &ViewHost<Fake>, share: f32) {
    pointer(platform, host, PointerPhase::Down, share);
    pointer(platform, host, PointerPhase::Up, share);
}

fn key(platform: &Platform, host: &ViewHost<Fake>, key: Key) {
    host.key(key, false, false);
    platform.settle(&[host]);
    run_frames(platform, host, 2);
}

#[test]
fn a_press_takes_the_nearer_thumb_and_a_drag_stops_at_the_other() {
    let worker = ViewWorker::spawn("test-range-press").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    tap(&platform, &host, 0.02);
    assert_eq!(
        values(&host),
        [0, 60],
        "a press at the start moves the lower"
    );
    tap(&platform, &host, 0.98);
    assert_eq!(
        values(&host),
        [0, 100],
        "a press at the end moves the upper"
    );
    tap(&platform, &host, 0.4);
    let [lower, upper] = values(&host);
    assert!(
        lower > 0 && upper == 100,
        "below the middle the lower is nearer"
    );
    tap(&platform, &host, 0.8);
    let [still, upper] = values(&host);
    assert!(
        still == lower && upper < 100,
        "above it the upper is nearer"
    );
    // The lower thumb, dragged past the upper, stops there; one finished change at the release.
    let before = text(&host, "log");
    pointer(&platform, &host, PointerPhase::Down, 0.4);
    for share in [0.5, 0.7, 0.9, 0.99] {
        pointer(&platform, &host, PointerPhase::Move, share);
    }
    assert_eq!(values(&host), [upper, upper], "they meet and do not cross");
    assert_eq!(
        text(&host, "log"),
        before,
        "held, the change is not finished"
    );
    pointer(&platform, &host, PointerPhase::Up, 0.99);
    assert_eq!(values(&host), [upper, upper]);
    assert_eq!(
        text(&host, "log"),
        format!("{before}{upper}-{upper} "),
        "the release finishes it once"
    );
    // Met, the side of the press picks the thumb, so neither is stuck.
    tap(&platform, &host, 0.3);
    let [lower, kept] = values(&host);
    assert!(lower < upper && kept == upper);
}

#[test]
fn keys_move_the_focused_thumb_and_assistive_tech_meets_two_sliders_in_a_group() {
    let worker = ViewWorker::spawn("test-range-keys").unwrap();
    let platform = Platform::new(&[&worker]);
    let host = page(&platform, &worker);
    let focused = |id: &str| query(&host, id)["focused"].as_bool().unwrap_or(false);
    key(&platform, &host, Key::Tab);
    assert!(focused("price-lower"), "the lower thumb is the first stop");
    key(&platform, &host, Key::Right);
    assert_eq!(values(&host), [30, 60]);
    assert_eq!(
        text(&host, "log"),
        "30-60 ",
        "a key step is finished at once"
    );
    for _ in 0..6 {
        key(&platform, &host, Key::Right);
    }
    assert_eq!(values(&host), [60, 60], "it stops at the upper thumb");
    key(&platform, &host, Key::Tab);
    assert!(focused("price-upper"), "the upper thumb is the next stop");
    key(&platform, &host, Key::Left);
    assert_eq!(values(&host), [60, 60], "which cannot go below the lower");
    key(&platform, &host, Key::Right);
    assert_eq!(values(&host), [60, 70]);
    let nodes = host.semantics(|projection, _| {
        let projection = projection.unwrap();
        (0..projection.frame.nodes.len())
            .filter_map(|index| projection.ids.id(index).and_then(|id| projection.node(id)))
            .map(|node| (node.role, node.name.clone(), node.range.clone()))
            .collect::<Vec<_>>()
    });
    assert!(
        nodes
            .iter()
            .any(|(role, name, _)| *role == Role::Group && name == "Price"),
        "{nodes:?}"
    );
    let sliders: Vec<_> = nodes
        .iter()
        .filter(|(role, ..)| *role == Role::Slider)
        .map(|(_, name, range)| {
            let range = range.as_ref().unwrap();
            (name.as_str(), range.min, range.value, range.max)
        })
        .collect();
    assert_eq!(
        sliders,
        [("From", 0.0, 60.0, 70.0), ("To", 60.0, 70.0, 100.0)],
        "each reports the values it can reach"
    );
}

#[test]
fn the_two_thumbs_share_one_track_and_the_range_lies_between_them() {
    use crate::scene::paint::Paint;
    let module = source(
        "function render(){return h('page',{},h('slider',{'aria-label':'Price',thumbLabels:['From','To'],values:[20,60],style:{width:224,height:40}}))}",
    );
    let display = crate::ViewSession::new(&module)
        .unwrap()
        .layout(400, 200, 1.0)
        .unwrap();
    let palette = crate::theme::Palette::builtin(false);
    let (line, accent) = (
        palette.role(lingxia_theme::Role::Line),
        palette.role(lingxia_theme::Role::Accent),
    );
    let rects: Vec<_> = display
        .commands
        .iter()
        .filter_map(|paint| match paint {
            Paint::Rect { color, bounds, .. } => Some((*color, *bounds)),
            _ => None,
        })
        .collect();
    let tracks: Vec<_> = rects.iter().filter(|(color, _)| *color == line).collect();
    assert_eq!(tracks.len(), 1, "one track");
    let [left, _, width, _] = tracks[0].1;
    // Thumbs are as tall as wide; the range is a thin line.
    let thumbs: Vec<f32> = rects
        .iter()
        .filter(|(color, bounds)| *color == accent && bounds[2] == bounds[3])
        .map(|(_, bounds)| bounds[0] + bounds[2] / 2.0)
        .collect();
    let ranges: Vec<_> = rects
        .iter()
        .filter(|(color, bounds)| *color == accent && bounds[2] != bounds[3])
        .map(|(_, bounds)| [bounds[0], bounds[0] + bounds[2]])
        .collect();
    let near = |a: f32, b: f32| (a - b).abs() < 0.01;
    let lines: Vec<f32> = rects
        .iter()
        .filter(|(color, bounds)| *color == accent && bounds[2] == bounds[3])
        .map(|(_, bounds)| bounds[1] + bounds[3] / 2.0)
        .collect();
    assert_eq!(thumbs.len(), 2);
    assert!(
        near(lines[0], lines[1]),
        "the thumbs ride one line: {lines:?}"
    );
    assert!(near(thumbs[0], left + width * 0.2) && near(thumbs[1], left + width * 0.6));
    assert_eq!(ranges.len(), 1, "one range");
    assert!(
        near(ranges[0][0], thumbs[0]) && near(ranges[0][1], thumbs[1]),
        "{ranges:?} {thumbs:?}"
    );
}
