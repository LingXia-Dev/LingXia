struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));
}
