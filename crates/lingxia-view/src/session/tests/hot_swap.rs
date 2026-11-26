//! Dev hot swap: a new module keeps local state; a broken one never replaces a working one.
use crate::ViewSession;

fn module(label: &str) -> String {
    super::source(&format!(
        "function render(){{const n=useLocal('n',0);return h('view',{{}},h('button',{{'data-testid':'b',onClick(){{n.set(n.value+1)}}}},'{label} '+n.value),h('scroll',{{'data-testid':'s',style:{{height:50}}}},h('view',{{style:{{height:300}}}})))}}"
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
    let broken = module("Total").replace("return h('view'", "throw Error('typo');return h('view'");
    let error = page.swap_module(&broken).unwrap_err();
    assert!(error.contains("typo"), "{error}");
    assert_eq!(text(&mut page), "Total 1");
    assert!(page.swap_module("not javascript (").is_err());
    assert_eq!(text(&mut page), "Total 1");
}

#[test]
fn a_swapped_module_drives_the_pages_own_players() {
    let mut page = ViewSession::new(&module("Count")).unwrap();
    page.swap_module(&module("Total")).unwrap();
    page.eval("__lingxiaVideoOrientation('landscape')").unwrap();
    assert_eq!(
        page.renderer.videos.orientation(),
        Some(crate::scene::video::Orientation::Landscape),
        "the new isolate's video globals reach this page's renderer"
    );
}

#[test]
fn component_locals_survive_a_swap_whatever_order_components_first_rendered_in() {
    // Two tab components; the page shows Paint first, then Inputs, whose local is set.
    let module = |label: &str| {
        super::source(&format!(
            "function Paint(){{const p=useLocal('p','paint');return h('text',{{}},p.value)}}
             function Inputs(){{const plan=useLocal('plan','initial');return h('button',{{'data-testid':'plan',onClick(){{plan.set('yearly')}}}},'{label} '+plan.value)}}
             function render(){{const tab=useLocal('tab','paint');return h('view',{{}},h('button',{{'data-testid':'tab',onClick(){{tab.set('inputs')}}}},'tab'),tab.value==='paint'?h(Paint,{{}}):h(Inputs,{{}}))}}"
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

/// Showcase page `page` (as `view-input`) built as `lingxia dev` builds it, as one module: these
/// sessions have no chunk loader.
pub(crate) fn showcase_page_module(page: &str) -> String {
    showcase_page_module_edited(page, None)
}

/// Showcase page `page` with one replacement in source `file` (relative to the lxapp) applied
/// to a copy, as a save in an editor would.
/// A save that adds a line to `ControlsDemo`, the component above the plan select on `view-controls`.
pub(crate) const SIBLING_EDIT: (&str, &str, &str) = (
    "shared/view-lab/elements.tsx",
    "<Readout testId=\"lab-controls-values\"",
    "<text className=\"text-xs\">Edited</text>\n      <Readout testId=\"lab-controls-values\"",
);

pub(crate) fn showcase_page_module_edited(page: &str, edit: Option<(&str, &str, &str)>) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let lab = std::path::Path::new(root).join("examples/lingxia-showcase/lxapp");
    assert!(
        lab.join("node_modules/@liulidesign/core").exists(),
        "the showcase imports Liuli from its node_modules: run npm install in {}",
        lab.display()
    );
    // Tests build in parallel; each build gets its own directory.
    static BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let tag = BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = std::env::temp_dir().join(format!("lingxia-showcase-{}-{tag}", std::process::id()));
    let entry = format!("pages/{page}/index.tsx");
    let mut source_root = lab.clone();
    if let Some((file, from, to)) = edit {
        let copy = out.join("src");
        for dir in [format!("pages/{page}"), "shared".into(), "theme".into()] {
            copy_tree(&lab.join(&dir), &copy.join(&dir));
        }
        std::fs::copy(lab.join("lxapp.json"), copy.join("lxapp.json")).unwrap();
        // Packages resolve from the project's own node_modules, as `lingxia dev` finds them.
        let modules = lab.join("node_modules");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&modules, copy.join("node_modules")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&modules, copy.join("node_modules")).unwrap();
        let source = copy.join(file);
        let text = std::fs::read_to_string(&source).unwrap();
        assert!(text.contains(from), "the edit applies to {file}");
        std::fs::write(&source, text.replacen(from, to, 1)).unwrap();
        source_root = copy;
    }
    let dist = out.join("dist");
    let messages = compiled_messages(&lab, &out.join("i18n"));
    let script = format!(
        "const {{ buildPages }} = await import('{root}/packages/lingxia-view/compiler/index.mjs'); \
         await buildPages({:?}, [{entry:?}], {:?}, {{}}, {{ dev: true, chunks: false, messages: {:?}, inputsRecord: {:?} }});",
        source_root,
        dist,
        messages,
        out.join("gpu-inputs.json")
    );
    let status = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .status()
        .expect("node builds the page");
    assert!(status.success(), "{entry} compiles");
    let module = std::fs::read_to_string(dist.join(format!("pages/{page}/index.js"))).unwrap();
    let _ = std::fs::remove_dir_all(&out);
    module
}

/// The showcase's catalogues compiled into `out` as the CLI compiles them; its `index.js`.
fn compiled_messages(lab: &std::path::Path, out: &std::path::Path) -> std::path::PathBuf {
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(lab.join("lxapp.json")).unwrap()).unwrap();
    let tags: Vec<&str> = manifest["languages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tag| tag.as_str().unwrap())
        .collect();
    std::fs::create_dir_all(out).unwrap();
    for (n, tag) in tags.iter().enumerate() {
        let json = std::fs::read_to_string(lab.join(format!("i18n/{tag}.json"))).unwrap();
        let catalogue = lingxia_i18n::Catalogue::parse(&json, n == 0).unwrap();
        std::fs::write(
            out.join(format!("{tag}.js")),
            lingxia_i18n::js::module(&catalogue),
        )
        .unwrap();
    }
    std::fs::write(
        out.join("index.js"),
        lingxia_i18n::js::index(tags[0], &tags),
    )
    .unwrap();
    out.join("index.js")
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
    let dev = showcase_page_module("view-controls");
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
        let plan = find(&display, "lab-input-plan");
        assert!(
            page.select_option(page.generation(), &plan.identity, 2)
                .unwrap()
        );
        // The swap lands before a frame commits the choice: pending state carries over too.
        page.swap_module(&dev).unwrap();
        let display = page.layout(400, 3000, 1.0).unwrap();
        let values = find(&display, "lab-select-values").text;
        assert_eq!(
            values.contains("Plan: yearly"),
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
fn a_showcase_select_keeps_a_chosen_plan_across_a_swap() {
    let module = showcase_page_module("view-controls");
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
    let plan = find(&display, "lab-input-plan");
    assert!(
        page.select_option(page.generation(), &plan.identity, 2)
            .unwrap()
    );
    let values = |page: &mut ViewSession| {
        let display = page.layout(400, 3000, 1.0).unwrap();
        find(&display, "lab-select-values").text
    };
    assert!(values(&mut page).contains("Plan: yearly"));
    page.swap_module(&module).unwrap();
    assert!(
        values(&mut page).contains("Plan: yearly"),
        "the Inputs tab and its plan survive"
    );
    // A save that edits a sibling component above Inputs.
    let edited = showcase_page_module_edited("view-controls", Some(SIBLING_EDIT));
    page.swap_module(&edited).unwrap();
    assert!(
        values(&mut page).contains("Plan: yearly"),
        "the plan survives an edit of a sibling component"
    );
    // Back to the original source, a second edit in a row.
    page.swap_module(&module).unwrap();
    assert!(
        values(&mut page).contains("Plan: yearly"),
        "and a second swap"
    );
    let display = page.layout(400, 3000, 1.0).unwrap();
    let plan = find(&display, "lab-input-plan");
    assert!(
        page.select_option(page.generation(), &plan.identity, 0)
            .unwrap()
    );
    assert!(values(&mut page).contains("Plan: monthly"));
    page.swap_module(&edited).unwrap();
    assert!(
        values(&mut page).contains("Plan: monthly"),
        "a choice made after a swap survives the next"
    );
}

/// A page whose `Card` component, declared as `name`, carries the compile-time id `id`.
fn identified(name: &str, id: &str) -> String {
    super::source(&format!(
        "function {name}(){{const n=useLocal('n',0);return h('button',{{'data-testid':'b',onClick(){{n.set(n.value+1)}}}},'Card '+n.value)}}\n\
         {name}.__lxid={id:?};\n\
         function render(){{return h('view',{{}},h({name},{{}}))}}"
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

#[test]
fn a_page_with_keyframe_animations_swaps_in_place() {
    use crate::session::lifecycle::SurfaceTicket;
    use std::time::Duration;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../packages/lingxia-view/tests/fixtures/animations.json"
    ))
    .unwrap();
    let payload = fixture["chart-in"].to_string();
    let module = |label: &str| {
        super::source(&format!(
            "function render(){{return h('view',{{}},h('view',{{style:{{width:40,height:40,animation:{payload}}}}}),h('text',{{'data-testid':'t'}},'{label}'))}}"
        )) + "\nenableHotSwap();"
    };
    let mut page = ViewSession::new(&module("before")).unwrap();
    page.set_animation_surface(Some(SurfaceTicket::new(1)), Duration::ZERO)
        .unwrap();
    let label = |page: &mut ViewSession, at: Duration| {
        page.prepare_view_animated(80, 80, 1.0, true, None, at, |display| {
            Ok(display
                .nodes
                .iter()
                .find(|n| n.test_id == "t")
                .unwrap()
                .text
                .clone())
        })
        .unwrap()
    };
    assert_eq!(label(&mut page, Duration::ZERO), "before");
    page.swap_module(&module("after")).unwrap();
    assert_eq!(label(&mut page, Duration::from_millis(16)), "after");
    // An animation the runtime cannot decode still refuses the swap.
    let broken = module("broken").replace(&payload, "'not an animation'");
    assert!(page.swap_module(&broken).is_err());
}
