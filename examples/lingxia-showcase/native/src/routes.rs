//! Routes in a private module: the device round calls them to prove `lingxia::host!`
//! registered them on each platform.

lingxia::route_defaults!(prefix = "showcase");

#[lingxia::native]
fn ping() -> lingxia::Result<String> {
    Ok("pong".to_string())
}

/// Seconds since the host addon was created: host-wide state lives on the addon.
#[lingxia::native]
fn uptime(host: lingxia::Host<crate::ExampleHostAddon>) -> u64 {
    host.started.elapsed().as_secs()
}
