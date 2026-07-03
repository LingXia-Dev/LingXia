use super::*;

pub(super) struct DevPortForward {
    cleanup: Option<PortForwardCleanup>,
}

enum PortForwardCleanup {
    Android { device: Option<String>, port: u16 },
    Harmony { device: Option<String>, port: u16 },
}

impl DevPortForward {
    pub(super) fn android(device: Option<&str>, port: u16) -> Result<Self> {
        let _ = run_adb_reverse_remove(device, port);
        run_adb_reverse(device, port)?;
        println!("  {} adb reverse tcp:{port} -> tcp:{port}", "✓".green());
        Ok(Self {
            cleanup: Some(PortForwardCleanup::Android {
                device: device.map(ToOwned::to_owned),
                port,
            }),
        })
    }

    pub(super) fn harmony(device: Option<&str>, port: u16) -> Result<Self> {
        let _ = run_hdc_reverse_remove(device, port);
        run_hdc_reverse(device, port)?;
        println!("  {} hdc rport tcp:{port} -> tcp:{port}", "✓".green());
        Ok(Self {
            cleanup: Some(PortForwardCleanup::Harmony {
                device: device.map(ToOwned::to_owned),
                port,
            }),
        })
    }
}

impl Drop for DevPortForward {
    fn drop(&mut self) {
        match self.cleanup.take() {
            Some(PortForwardCleanup::Android { device, port }) => {
                let _ = run_adb_reverse_remove(device.as_deref(), port);
            }
            Some(PortForwardCleanup::Harmony { device, port }) => {
                let _ = run_hdc_reverse_remove(device.as_deref(), port);
            }
            None => {}
        }
    }
}

fn adb_command(device: Option<&str>) -> Command {
    let mut command = Command::new("adb");
    if let Some(device) = device {
        command.arg("-s").arg(device);
    }
    command
}

fn run_adb_reverse(device: Option<&str>, port: u16) -> Result<()> {
    let output = adb_command(device)
        .args(["reverse", &format!("tcp:{port}"), &format!("tcp:{port}")])
        .output()
        .context("Failed to execute adb reverse")?;
    ensure_command_success(output, "adb reverse")
}

fn run_adb_reverse_remove(device: Option<&str>, port: u16) -> Result<()> {
    let output = adb_command(device)
        .args(["reverse", "--remove", &format!("tcp:{port}")])
        .output()
        .context("Failed to execute adb reverse --remove")?;
    ensure_command_success(output, "adb reverse --remove")
}

fn hdc_command(device: Option<&str>) -> Command {
    let mut command = Command::new("hdc");
    if let Some(device) = device {
        command.arg("-t").arg(device);
    }
    command
}

fn run_hdc_reverse(device: Option<&str>, port: u16) -> Result<()> {
    // A dev session that died uncleanly leaves its reverse rule behind, and
    // the device-side listener it holds makes the re-added rule report OK
    // while `uv_listen` fails on-device. Clear it first; ignore "not found".
    let _ = hdc_command(device)
        .args([
            "fport",
            "rm",
            &format!("tcp:{port}"),
            &format!("tcp:{port}"),
        ])
        .output();
    let output = hdc_command(device)
        .args(["rport", &format!("tcp:{port}"), &format!("tcp:{port}")])
        .output()
        .context("Failed to execute hdc rport")?;
    ensure_command_success(output, "hdc rport")
}

fn run_hdc_reverse_remove(device: Option<&str>, port: u16) -> Result<()> {
    let output = hdc_command(device)
        .args(["fport", "rm", &format!("tcp:{port} tcp:{port}")])
        .output()
        .context("Failed to execute hdc fport rm")?;
    ensure_command_success(output, "hdc fport rm")
}
