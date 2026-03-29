#[cfg(feature = "simulator-ui")]
fn main() -> Result<(), String> {
    let runtime = info_panel_lib::simulator::InfoPanelSimulatorRuntime::new();
    simulator::ui::run_editor(&runtime, std::path::Path::new("."))
}

#[cfg(not(feature = "simulator-ui"))]
fn main() {}
