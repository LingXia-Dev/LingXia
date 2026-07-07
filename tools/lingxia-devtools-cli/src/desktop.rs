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
    /// Capture a display, window, or region (defaults to the whole screen)
    Screenshot {
        /// Capture a monitor by 1-based index (from `desktop displays`)
        #[arg(long)]
        display: Option<usize>,
        /// Capture a window by id (occlusion-independent)
        #[arg(long)]
        window: Option<String>,
        /// Capture a region as X,Y,W,H in global physical pixels
        #[arg(long)]
        region: Option<String>,
        /// Output path; `-` for stdout. Default: .lingxia/screenshots/desktop-<ts>.png
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Print the JSON envelope (metadata + base64 PNG)
        #[arg(long)]
        json: bool,
    },
    /// Read the color of a pixel at a screen coordinate
    Pixel {
        /// Coordinate as X,Y in global physical pixels
        #[arg(long)]
        at: String,
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
        DesktopCommand::Screenshot {
            display,
            window,
            region,
            output,
            json,
        } => run_screenshot(display, window, region, output, json),
        DesktopCommand::Pixel { at, json } => {
            let (x, y) = match parse_pair(&at) {
                Ok(p) => p,
                Err(e) => finish::<()>(json, Err(e), |_| {}),
            };
            finish(json, cu::pixel(x, y), print_pixel)
        }
    }
}

/// `X,Y` -> (i32, i32).
fn parse_pair(s: &str) -> cu::Result<(i32, i32)> {
    let (a, b) = s
        .split_once(',')
        .ok_or_else(|| cu::Error::Usage(format!("expected X,Y, got '{s}'")))?;
    Ok((
        a.trim()
            .parse()
            .map_err(|_| cu::Error::Usage(format!("invalid X in '{s}'")))?,
        b.trim()
            .parse()
            .map_err(|_| cu::Error::Usage(format!("invalid Y in '{s}'")))?,
    ))
}

fn run_screenshot(
    display: Option<usize>,
    window: Option<String>,
    region: Option<String>,
    output: Option<String>,
    json: bool,
) -> ! {
    let selectors = display.is_some() as u8 + window.is_some() as u8 + region.is_some() as u8;
    if selectors > 1 {
        finish::<()>(
            json,
            Err(cu::Error::Usage(
                "pass at most one of --display / --window / --region".into(),
            )),
            |_| {},
        );
    }
    let target = if let Some(n) = display {
        cu::CaptureTarget::Display(n)
    } else if let Some(id) = window {
        cu::CaptureTarget::Window(id)
    } else if let Some(r) = region {
        match parse_region(&r) {
            Ok(t) => t,
            Err(e) => finish::<()>(json, Err(e), |_| {}),
        }
    } else {
        cu::CaptureTarget::Screen
    };

    let capture = match cu::screenshot(target) {
        Ok(c) => c,
        Err(e) => finish::<()>(json, Err(e), |_| {}),
    };

    if json {
        use base64::Engine as _;
        let envelope = serde_json::json!({
            "target": "desktop",
            "kind": "screenshot",
            "coordinate_space": "desktop_pixels",
            "backend": capture.backend,
            "occlusion_independent": capture.occlusion_independent,
            "format": "png",
            "width": capture.width,
            "height": capture.height,
            "image": {
                "mime": "image/png",
                "encoding": "base64",
                "data": base64::engine::general_purpose::STANDARD.encode(&capture.png),
            }
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).unwrap_or_default()
        );
        std::process::exit(0);
    }

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    match crate::screenshot::write_png(output, format!("desktop-{ts}.png"), &capture.png) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(10);
        }
    }
}

fn parse_region(s: &str) -> cu::Result<cu::CaptureTarget> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err(cu::Error::Usage(format!("expected X,Y,W,H, got '{s}'")));
    }
    let n = |v: &str| {
        v.parse::<i32>()
            .map_err(|_| cu::Error::Usage(format!("invalid number in region '{s}'")))
    };
    Ok(cu::CaptureTarget::Region {
        x: n(parts[0])?,
        y: n(parts[1])?,
        w: n(parts[2])?,
        h: n(parts[3])?,
    })
}

fn print_pixel(p: &cu::Pixel) {
    println!("#{}  rgb({},{},{})  at {},{}", p.hex, p.r, p.g, p.b, p.x, p.y);
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
