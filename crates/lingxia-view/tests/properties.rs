//! Property tests over generated wire buffers: no panic, no non-finite geometry, and
//! visible bounds inside the viewport for every buffer the decoder admits.
//!   LINGXIA_PROPERTY_CASES=N (default 400) · LINGXIA_PROPERTY_SEED=S replays one run.

#[path = "support/invariants.rs"]
mod invariants;
#[path = "support/wire_gen.rs"]
mod wire_gen;

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// SplitMix64: deterministic bytes per case.
fn bytes(seed: u64, case: u64) -> Vec<u8> {
    let mut state = seed ^ case.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut next = || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let len = 16 + (next() % 2048) as usize;
    (0..len).map(|_| next() as u8).collect()
}

fn run(data: &[u8]) -> Result<bool, String> {
    let wire = wire_gen::buffer(data);
    let (width, height, scale) = wire_gen::viewport(data);
    std::panic::catch_unwind(|| invariants::check_wire(&wire, width, height, scale)).unwrap_or_else(
        |panic| {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("panic: {message}"))
        },
    )
}

/// Drops byte ranges while the failure persists, so the report names a small input.
fn shrink(mut data: Vec<u8>) -> Vec<u8> {
    let mut chunk = data.len() / 2;
    while chunk > 0 {
        let mut start = 0;
        while start + chunk <= data.len() {
            let mut smaller = data.clone();
            smaller.drain(start..start + chunk);
            if run(&smaller).is_err() {
                data = smaller;
            } else {
                start += chunk;
            }
        }
        chunk /= 2;
    }
    data
}

#[test]
fn admitted_buffers_lay_out_to_finite_geometry_inside_the_viewport() {
    let seed = env("LINGXIA_PROPERTY_SEED", 0x11_9a_71);
    let cases = env("LINGXIA_PROPERTY_CASES", 400);
    let mut admitted = 0;
    for case in 0..cases {
        let data = bytes(seed, case);
        match run(&data) {
            Ok(true) => admitted += 1,
            Ok(false) => {}
            Err(error) => {
                let small = shrink(data);
                let wire = wire_gen::buffer(&small);
                panic!(
                    "seed {seed} case {case}: {error}\nviewport {:?}\nwire {}",
                    wire_gen::viewport(&small),
                    serde_json::to_string(&wire).unwrap()
                );
            }
        }
    }
    // The generator must keep reaching layout, or the property says nothing.
    assert!(
        admitted * 4 >= cases,
        "only {admitted} of {cases} buffers were admitted"
    );
}

/// Found by the property above: a stretched row whose margin exceeds its parent.
#[test]
fn a_margin_wider_than_the_parent_never_yields_a_negative_size() {
    let wire: Vec<serde_json::Value> = serde_json::from_str(
        r#"[0,-1,"","root","",0,1,0,"row","row","",1,112,71.0,4,6,"gap","gap","",0]"#,
    )
    .unwrap();
    let records = lingxia_view::protocol::decode(&wire).unwrap();
    let frame = lingxia_view::Renderer::default()
        .layout(&records, 64, 64, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[1].bounds, [71.0, 0.0, 0.0, 0.0]);
}

#[test]
fn the_fuzz_automation_page_decodes_and_lays_out() {
    let wire: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fuzz/page.json")).unwrap();
    let records = lingxia_view::protocol::decode(&wire).unwrap();
    lingxia_view::Renderer::default()
        .layout(&records, 320, 480, 1.0)
        .unwrap();
}
