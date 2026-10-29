//! Dev hot swap: a new module keeps local state; a broken one never replaces a working one.
use crate::ViewSession;

fn module(label: &str) -> String {
    super::source(&format!(
        "function render(){{const n=useLocal('n',0);return h('column',{{}},h('button',{{'data-testid':'b',onClick(){{n.set(n.value+1)}}}},'{label} '+n.value),h('scroll',{{'data-testid':'s',style:{{height:50}}}},h('column',{{style:{{height:300}}}})))}}"
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
             function Inputs(){{const plan=useLocal('plan','initial');return h('button',{{'data-testid':'plan',onClick(){{plan.set('yearly')}}}},'{label} '+plan.value)}}
             function render(){{const tab=useLocal('tab','paint');return h('column',{{}},h('button',{{'data-testid':'tab',onClick(){{tab.set('inputs')}}}},'tab'),tab.value==='paint'?h(Paint,{{}}):h(Inputs,{{}}))}}"
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
    // Tests build in parallel; each build gets its own directory.
    static BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let tag = BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

/// `module` minified as a release package ships it: top-level identifiers mangled, with or
/// without function names kept.
fn minified(module: &str, keep_names: bool) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let input = std::env::temp_dir().join(format!(
        "lingxia-minify-{}-{keep_names}.js",
        std::process::id()
    ));
    std::fs::write(&input, module).unwrap();
    let script = format!(
        "const {{ build }} = await import('{root}/packages/node_modules/esbuild/lib/main.js'); \
         const out = await build({{ entryPoints: [{:?}], write: false, minify: true, keepNames: {keep_names}, format: 'esm', bundle: false }}); \
         process.stdout.write(out.outputFiles[0].text);",
        input
    );
    let out = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let header = module.lines().next().unwrap();
    format!("{header}\n{}", String::from_utf8(out.stdout).unwrap())
}

#[test]
fn a_packaged_view_keeps_its_state_through_the_first_dev_swap() {
    let dev = view_lab_module();
    {
        let (keep_names, survives) = (false, true);
        let mut page = ViewSession::new(&minified(&dev, keep_names)).unwrap();
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
        // The swap lands before a frame commits the choice: pending state carries over too.
        page.swap_module(&dev).unwrap();
        let display = page.layout(400, 3000, 1.0).unwrap();
        let values = find(&display, "lab-input-values").text;
        assert_eq!(
            values.ends_with("plan=yearly"),
            survives,
            "keep_names={keep_names}: {values}"
        );
    }
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

/// A page whose `Card` component, declared as `name`, carries the compile-time id `id`.
fn identified(name: &str, id: &str) -> String {
    super::source(&format!(
        "function {name}(){{const n=useLocal('n',0);return h('button',{{'data-testid':'b',onClick(){{n.set(n.value+1)}}}},'Card '+n.value)}}\n\
         {name}.__lxid={id:?};\n\
         function render(){{return h('column',{{}},h({name},{{}}))}}"
    )) + "\nenableHotSwap();"
}

#[test]
fn component_state_follows_the_compile_time_id_not_the_function_name() {
    let mut page = ViewSession::new(&identified("Card", "pages/a/index#Card")).unwrap();
    let handle = page
        .layout(200, 200, 1.0)
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.test_id == "b")
        .unwrap()
        .on_click
        .unwrap();
    assert!(page.click(page.generation(), handle).unwrap());
    assert_eq!(text(&mut page), "Card 1");
    // A minifier renamed the function; the id it was compiled with is unchanged.
    page.swap_module(&identified("q", "pages/a/index#Card"))
        .unwrap();
    assert_eq!(
        text(&mut page),
        "Card 1",
        "a renamed function keeps its scope"
    );
    // The declaration moved to another module: a new id is a new component.
    page.swap_module(&identified("Card", "shared/card#Card"))
        .unwrap();
    assert_eq!(
        text(&mut page),
        "Card 0",
        "a moved declaration starts fresh"
    );
}
