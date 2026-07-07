use clap::{Args, Subcommand};
use lingxia_computer_use as cu;
use serde::Serialize;

#[derive(Args, Clone)]
pub struct DesktopOptions {
    #[command(subcommand)]
    command: DesktopCommand,
}

#[derive(Subcommand, Clone)]
pub enum DesktopCommand {
    /// Report backend, capabilities, and permission status
    Doctor {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// List monitors/displays (global physical pixels)
    Displays {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// List local OS windows
    Windows {
        /// Match query: bare text, or a title:/class:/process:/pid: prefix
        #[arg(long = "match")]
        match_query: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(options: DesktopOptions) -> ! {
    match options.command {
        DesktopCommand::Doctor { json } => finish(json, Ok(cu::doctor()), print_doctor),
        DesktopCommand::Displays { json } => finish(json, cu::displays(), print_displays),
        DesktopCommand::Windows { match_query, json } => {
            let query = match_query
                .as_deref()
                .map(cu::WindowQuery::parse)
                .unwrap_or_default();
            finish(json, cu::windows(&query), print_windows)
        }
    }
}

/// Emit the result and exit with the contract's exit code. `desktop` commands
/// run locally (no dev session), so they own their process exit directly.
fn finish<T: Serialize>(json: bool, result: cu::Result<T>, human: impl Fn(&T)) -> ! {
    match result {
        Ok(value) => {
            if json {
                match serde_json::to_string_pretty(&value) {
                    Ok(text) => println!("{text}"),
                    Err(err) => {
                        eprintln!("Error: failed to serialize output: {err}");
                        std::process::exit(10);
                    }
                }
            } else {
                human(&value);
            }
            std::process::exit(0);
        }
        Err(err) => {
            if json {
                let envelope = serde_json::json!({
                    "error": {
                        "code": err.code(),
                        "message": err.to_string(),
                        "exit_code": err.exit_code(),
                    }
                });
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&envelope).unwrap_or_default()
                );
            } else {
                eprintln!("Error: {err}");
            }
            std::process::exit(err.exit_code());
        }
    }
}

fn yn(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn print_doctor(d: &cu::Doctor) {
    println!("backend    {}", d.backend);
    println!("os         {} {}", d.os, d.os_version);
    let c = &d.capabilities;
    println!("capabilities:");
    println!("  displays            {}", yn(c.displays));
    println!("  windows             {}", yn(c.windows));
    println!("  screenshot          {}", yn(c.screenshot));
    println!("  pixel               {}", yn(c.pixel));
    println!("  pointer             {}", yn(c.pointer));
    println!("  key                 {}", yn(c.key));
    println!("  window management   {}", yn(c.window_management));
    println!("  clipboard           {}", yn(c.clipboard));
    println!("  ax tree             {}", yn(c.ax_tree));
    println!("  ocr                 {}", yn(c.ocr));
}

fn print_displays(displays: &Vec<cu::Display>) {
    if displays.is_empty() {
        println!("No displays reported.");
        return;
    }
    println!(
        "{:<10}  {:<7}  {:<20}  {:<6}  DPI",
        "ID", "PRIMARY", "BOUNDS", "SCALE"
    );
    for d in displays {
        println!(
            "{:<10}  {:<7}  {:<20}  {:<6}  {}",
            d.id,
            yn(d.primary),
            format!("{},{} {}x{}", d.bounds.x, d.bounds.y, d.bounds.w, d.bounds.h),
            format!("{:.2}", d.scale),
            d.dpi,
        );
    }
}

fn print_windows(windows: &Vec<cu::Window>) {
    if windows.is_empty() {
        println!("No matching windows.");
        return;
    }
    println!(
        "{:<12}  {:<6}  {:<18}  {:<19}  {:<3}  TITLE",
        "ID", "PID", "PROCESS", "BOUNDS", "FOC"
    );
    for w in windows {
        println!(
            "{:<12}  {:<6}  {:<18}  {:<19}  {:<3}  {}",
            w.id,
            w.pid,
            truncate(&w.process, 18),
            format!("{},{} {}x{}", w.bounds.x, w.bounds.y, w.bounds.w, w.bounds.h),
            yn(w.focused),
            truncate(&w.title, 60),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
