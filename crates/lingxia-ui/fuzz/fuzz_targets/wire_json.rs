//! Raw JSON from the bridge straight into the decoder: every field id, every payload shape.
#![no_main]

#[path = "../../tests/support/invariants.rs"]
mod invariants;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(wire) = serde_json::from_slice::<Vec<serde_json::Value>>(data) else {
        return;
    };
    if let Err(error) = invariants::check_wire(&wire, 200, 300, 2.0) {
        panic!("{error}");
    }
});
