struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    // Cloud provider. Must register in this hook — the logic context is built
    // before `start_services`. Injected via `--with-provider cloud`. The runner
    // env contract (config.toml overrides, mock dir, functions.json routing) is
    // resolved by `lingxia_runner_config`, shared with the Windows runner.
    #[cfg(feature = "cloud")]
    fn install_logic_extensions(&self) {
        if let Err(err) = lingxia_cloud_client::init(cloud_options()) {
            eprintln!("[cloud] provider init failed: {err}");
        }
    }

    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

/// Map the shared, cloud-free runner config onto the cloud client's option and
/// routing types (available only here, via the injected provider crate).
#[cfg(feature = "cloud")]
fn cloud_options() -> lingxia_cloud_client::CloudOptions {
    use lingxia_cloud_client::{CloudOptions, MockRouting, Provider};
    let cfg = lingxia_runner_config::from_env();
    let mut options = CloudOptions::default();
    if let Some(server) = cfg.lingxia_server {
        options = options.lingxia_server(server);
    }
    if let Some(id) = cfg.lingxia_id {
        options = options.lingxia_id(id);
    }
    if let Some(mock) = cfg.mock {
        let provider = |live| if live { Provider::Live } else { Provider::Mock };
        let routing = MockRouting {
            default: provider(mock.routing.default_live),
            overrides: mock
                .routing
                .overrides
                .into_iter()
                .map(|(name, live)| (name, provider(live)))
                .collect(),
        };
        options = options.lingxiao_mock(mock.dir).lingxiao_routing(routing);
    }
    options
}

#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));
}
