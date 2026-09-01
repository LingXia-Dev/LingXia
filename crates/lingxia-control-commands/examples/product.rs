//! What a shipped product's command line looks like.
//!
//! A product binary mounts these subcommands the same way `lxdev` does, but
//! reaches its own running instance over the local control socket instead of a
//! dev session. Run it against a live app:
//!
//! ```text
//! cargo run -p lingxia-control-commands --example product -- \
//!     --endpoint "$HOME/Library/Application Support/<app-id>/lingxia/control/control.sock" \
//!     app doctor
//! ```

use clap::{Parser, Subcommand};
use lingxia_control_commands::{app, desktop, transport::ControlSocket};

#[derive(Parser)]
#[command(name = "product", about = "A product's own command line")]
struct Cli {
    /// The control socket, as the product's `local_control::endpoint_name` reports
    /// it. A real product computes this itself rather than taking a flag.
    #[arg(long, global = true)]
    endpoint: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Automate the machine. Runs inside the product, not here.
    Desktop(desktop::DesktopOptions),
    /// Drive this product's own windows.
    App(app::AppOptions),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let endpoint = cli
        .endpoint
        .ok_or_else(|| anyhow::anyhow!("--endpoint is required"))?;
    let transport = ControlSocket::at(endpoint);
    match cli.command {
        // Sent to the product rather than run here: macOS attributes
        // Accessibility and Screen Recording to the responsible process, so
        // running them in this process would borrow the terminal's grants.
        Command::Desktop(options) => std::process::exit(desktop::execute(
            &desktop::Backend::App(&transport),
            options,
        )),
        Command::App(options) => {
            let context = app::AppContext {
                transport: &transport,
                target: std::env::consts::OS.to_string(),
                session: None,
            };
            app::execute(&context, options)
        }
    }
}
