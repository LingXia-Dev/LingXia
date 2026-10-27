//! Dev hot swap: a new module keeps local state; a broken one never replaces a working one.
use crate::ViewSession;

fn module(label: &str) -> String {
    super::source(&format!(
        "function render(){{const n=useLocal('n',0);return h('column',{{}},h('button',{{testId:'b',onClick(){{n.set(n.value+1)}}}},'{label} '+n.value),h('scroll',{{testId:'s',style:{{height:50}}}},h('column',{{style:{{height:300}}}})))}}"
    )) + "\nenableHotSwap();"
}

fn text(page: &mut ViewSession) -> String {
    let display = page.layout(200, 200, 1.0).unwrap();
    display
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .text
        .clone()
}

#[test]
fn a_swapped_module_keeps_local_state_and_a_broken_one_rolls_back() {
    let mut page = ViewSession::new(&module("Count")).unwrap();
    let first = page.layout(200, 200, 1.0).unwrap();
    let handle = first
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), handle).unwrap());
    assert_eq!(text(&mut page), "Count 1");
    page.swap_module(&module("Total")).unwrap();
    assert_eq!(text(&mut page), "Total 1");
    // A module that throws while rendering leaves the running one in place.
    let broken =
        module("Total").replace("return h('column'", "throw Error('typo');return h('column'");
    let error = page.swap_module(&broken).unwrap_err();
    assert!(error.contains("typo"), "{error}");
    assert_eq!(text(&mut page), "Total 1");
    assert!(page.swap_module("not javascript (").is_err());
    assert_eq!(text(&mut page), "Total 1");
}

#[test]
fn component_locals_survive_a_swap_whatever_order_components_first_rendered_in() {
    // Two tab components; the page shows Paint first, then Inputs, whose local is set.
    let module = |label: &str| {
        super::source(&format!(
            "function Paint(){{const p=useLocal('p','paint');return h('text',{{}},p.value)}}
             function Inputs(){{const plan=useLocal('plan','initial');return h('button',{{testId:'plan',onClick(){{plan.set('yearly')}}}},'{label} '+plan.value)}}
             function render(){{const tab=useLocal('tab','paint');return h('column',{{}},h('button',{{testId:'tab',onClick(){{tab.set('inputs')}}}},'tab'),tab.value==='paint'?h(Paint,{{}}):h(Inputs,{{}}))}}"
        )) + "\nenableHotSwap();"
    };
    let mut page = ViewSession::new(&module("Plan")).unwrap();
    let press = |page: &mut ViewSession, id: &str| {
        let display = page.layout(200, 200, 1.0).unwrap();
        let handle = display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .on_click
            .unwrap();
        assert!(page.click(page.generation(), handle).unwrap());
    };
    press(&mut page, "tab");
    press(&mut page, "plan");
    let shown = |page: &mut ViewSession| {
        let display = page.layout(200, 200, 1.0).unwrap();
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "plan")
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(shown(&mut page), "Plan yearly");
    // The new isolate meets Inputs before Paint; the local must still be found.
    page.swap_module(&module("Billing")).unwrap();
    assert_eq!(shown(&mut page), "Billing yearly");
}

/// The showcase view lab compiled as `lingxia dev` compiles it (dev, hot swap on).
fn view_lab_module() -> String {
    view_lab_module_edited(None)
}

/// The view lab with one source replacement applied to a copy, as a save in an editor would.
pub(crate) fn view_lab_module_edited(edit: Option<(&str, &str)>) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let lab = std::path::Path::new(root).join("examples/lingxia-showcase-gpu/lxapp");
    let tag = edit.map_or(0, |(from, _)| from.len());
    let out = std::env::temp_dir().join(format!("lingxia-view-lab-{}-{tag}", std::process::id()));
    let mut source_root = lab.clone();
    if let Some((from, to)) = edit {
        let copy = out.join("src");
        for dir in ["pages/view-lab", "shared", "theme"] {
            copy_tree(&lab.join(dir), &copy.join(dir));
        }
        std::fs::copy(lab.join("lxapp.json"), copy.join("lxapp.json")).unwrap();
        let page = copy.join("pages/view-lab/index.tsx");
        let text = std::fs::read_to_string(&page).unwrap();
        assert!(text.contains(from), "the edit applies");
        std::fs::write(&page, text.replacen(from, to, 1)).unwrap();
        source_root = copy;
    }
    let dist = out.join("dist");
    let script = format!(
        "const {{ buildPages }} = await import('{root}/packages/lingxia-ui/compiler/index.mjs'); \
         await buildPages({:?}, ['pages/view-lab/index.tsx'], {:?}, {{}}, {{ dev: true }});",
        source_root, dist
    );
    let status = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .status()
        .expect("node builds the view lab");
    assert!(status.success(), "the view lab compiles");
    let module = std::fs::read_to_string(dist.join("pages/view-lab/index.js")).unwrap();
    let _ = std::fs::remove_dir_all(&out);
    module
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn the_view_lab_keeps_a_chosen_plan_across_a_swap() {
    let module = view_lab_module();
    let mut page = ViewSession::new(&module).unwrap();
    let find = |display: &crate::DisplayList, id: &str| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .cloned()
            .unwrap()
    };
    let display = page.layout(400, 3000, 1.0).unwrap();
    let tab = find(&display, "lab-tab-input").on_click.unwrap();
    assert!(page.click(page.generation(), tab).unwrap());
    let display = page.layout(400, 3000, 1.0).unwrap();
    let plan = find(&display, "lab-input-plan");
    assert!(
        page.select_option(page.generation(), &plan.identity, 2)
            .unwrap()
    );
    let values = |page: &mut ViewSession| {
        let display = page.layout(400, 3000, 1.0).unwrap();
        find(&display, "lab-input-values").text
    };
    assert!(values(&mut page).ends_with("plan=yearly"));
    page.swap_module(&module).unwrap();
    assert!(
        values(&mut page).ends_with("plan=yearly"),
        "the Inputs tab and its plan survive"
    );
    // A save that edits a sibling component above Inputs.
    let edited = view_lab_module_edited(Some((
        "Monthly bills</text>",
        "Monthly bills and more</text>\n      <text className=\"text-xs\">Edited</text>",
    )));
    page.swap_module(&edited).unwrap();
    assert!(
        values(&mut page).ends_with("plan=yearly"),
        "the plan survives an edit of a sibling component"
    );
    // Back to the original source, a second edit in a row.
    page.swap_module(&module).unwrap();
    assert!(
        values(&mut page).ends_with("plan=yearly"),
        "and a second swap"
    );
    let display = page.layout(400, 3000, 1.0).unwrap();
    let plan = find(&display, "lab-input-plan");
    assert!(
        page.select_option(page.generation(), &plan.identity, 0)
            .unwrap()
    );
    assert!(values(&mut page).ends_with("plan=monthly"));
    page.swap_module(&edited).unwrap();
    assert!(
        values(&mut page).ends_with("plan=monthly"),
        "a choice made after a swap survives the next"
    );
}
