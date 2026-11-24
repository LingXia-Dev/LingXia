//! A route in a private module: the device round calls it to prove `lingxia::host!`
//! registered it on each platform.

#[lingxia::native("showcase.ping")]
fn ping() -> lingxia::Result<String> {
    Ok("pong".to_string())
}
