//! Devices subcommand for Apple Developer Services.

use anyhow::Result;
use colored::Colorize;

use super::client::with_client;

/// Execute devices command
pub fn execute() -> Result<()> {
    with_client(|client| {
        let devices = client.list_devices()?;

        if devices.is_empty() {
            println!("{}", "No registered devices found.".yellow());
            return Ok(());
        }

        println!("{}", "Registered Devices".cyan().bold());
        println!();

        for device in devices {
            println!("- id: {}", device.id.bold());

            if let Some(name) = &device.name {
                println!("  name: {}", name);
            }

            println!("  udid: {}", device.udid);

            if let Some(platform) = &device.platform {
                println!("  platform: {}", platform);
            }

            if let Some(device_class) = &device.device_class {
                println!("  device class: {}", device_class);
            }

            if let Some(status) = &device.status {
                let status_colored = match status.as_str() {
                    "c" => "enabled".green(),
                    _ => status.yellow(),
                };
                println!("  status: {}", status_colored);
            }

            if let Some(model) = &device.model {
                println!("  model: {}", model);
            }

            if let Some(added_date) = &device.added_date {
                println!("  added date: {}", added_date);
            }
        }

        Ok(())
    })
}
