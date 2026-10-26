//! Structured wire buffers (mostly admitted) through decode, layout and raster.
#![no_main]

#[path = "../../tests/support/invariants.rs"]
mod invariants;
#[path = "../../tests/support/wire_gen.rs"]
mod wire_gen;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let wire = wire_gen::buffer(data);
    let (width, height, scale) = wire_gen::viewport(data);
    if let Err(error) = invariants::check_wire(&wire, width, height, scale) {
        panic!("{error}\n{}", serde_json::to_string(&wire).unwrap());
    }
});
