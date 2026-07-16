use crate::appicon;
use crate::config::{HOST_CONFIG_FILE, LingXiaConfig, has_host_config};
use crate::platform;
use crate::platform::detector::PlatformType;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Execute the icon command to generate or update app icons
pub fn execute(
    icon_path: String,
    platform: Option<String>,
    background_color: Option<String>,
    legacy: bool,
    foreground: Option<String>,
    output: Option<String>,
    size: Option<u32>,
    check: bool,
) -> Result<()> {
    println!("{}", "Generate/Update App Icons".bold());
    println!();

    // Check if icon file exists
    let current_dir = std::env::current_dir()?;
    let icon_path = current_dir.join(&icon_path);
    if !icon_path.exists() {
        return Err(anyhow!("Icon file not found: {:?}", icon_path));
    }

    // Standalone conversion (no project): write the source to `--output` as a
    // committed asset, format chosen by the extension. Keeps asset specifics out
    // of the CLI — the caller passes the source/output/size.
    if let Some(output) = output {
        return write_converted_icon(&icon_path, &current_dir.join(&output), size);
    }

    let foreground_path = match &foreground {
        Some(p) => {
            let path = current_dir.join(p);
            if !path.exists() {
                return Err(anyhow!("Foreground icon file not found: {:?}", path));
            }
            Some(path)
        }
        None => None,
    };

    // Validate the explicit background early; `None` means auto-detect from the
    // source's flat backdrop (resolve_layer_plan hard-stops when it can't).
    let background_color = background_color
        .as_deref()
        .map(appicon::normalize_android_color)
        .transpose()?;

    // Preview-only mode: analyze, render every platform treatment into
    // icon-preview.html, write nothing into any project.
    if check {
        return run_check_preview(
            &icon_path,
            foreground_path.as_deref(),
            background_color.as_deref(),
        );
    }

    let context = resolve_icon_context(&current_dir)?;

    // Determine target platform(s)
    let platforms: Vec<String> = if let Some(p) = platform {
        vec![p.to_lowercase()]
    } else {
        context
            .config
            .as_ref()
            .and_then(|cfg| cfg.app.as_ref())
            .as_ref()
            .map(|app| app.platforms.clone())
            .or_else(|| context.inferred_platform.as_ref().map(|p| vec![p.as_str().to_string()]))
            .ok_or_else(|| {
                anyhow!(
                    "Failed to determine target platform. Pass --platform or run this command from a LingXia host project or Apple Swift Package directory."
                )
            })?
    };

    if platforms.is_empty() {
        return Err(anyhow!(
            "No platforms specified. Please specify a platform using --platform or configure app.platforms in lingxia.yaml"
        ));
    }

    println!(
        "  Icon source:      {}",
        icon_path.display().to_string().cyan()
    );
    println!("  Target platform:  {}", platforms.join(", ").cyan());
    println!(
        "  Background color: {}",
        background_color
            .as_deref()
            .unwrap_or("(auto-detect from source)")
            .cyan()
    );
    if legacy {
        println!("  Legacy support:   {}", "enabled (minSdk < 26)".cyan());
    }
    if let Some(p) = &foreground_path {
        println!("  Foreground:       {}", p.display().to_string().cyan());
    }
    println!();

    // Icons are brand assets — confirm before overwriting project resources.
    // `--check` renders a preview without touching anything.
    if std::io::stdin().is_terminal() {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!("Write icons for {}?", platforms.join(", ")))
            .default(true)
            .interact()
            .unwrap_or(true);
        if !confirmed {
            println!("Aborted — nothing written. Tip: `--check` renders a preview first.");
            return Ok(());
        }
        println!();
    }

    let mut generated_count = 0;
    let app_project_name = context
        .config
        .as_ref()
        .and_then(|cfg| cfg.app.as_ref())
        .map(|a| a.project_name.as_str());

    for platform_name in platforms {
        match platform_name.as_str() {
            "android" => {
                println!("{}", "Generating Android icons...".bold());
                match platform::android::generate_icons(
                    &context.project_root,
                    &icon_path,
                    background_color.as_deref(),
                    legacy,
                    foreground_path.as_deref(),
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping Android icon generation.");
                    }
                }
            }
            "ios" => {
                println!("{}", "Generating iOS icons...".bold());
                match platform::ios::generate_icons(
                    &context.project_root,
                    &icon_path,
                    context.config.as_ref().and_then(|cfg| cfg.ios.as_ref()),
                    app_project_name,
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping iOS icon generation.");
                    }
                }
            }
            "macos" => {
                println!("{}", "Generating macOS icons...".bold());
                match platform::macos::generate_icons(
                    &context.project_root,
                    &icon_path,
                    context.config.as_ref().and_then(|cfg| cfg.macos.as_ref()),
                    app_project_name,
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping macOS icon generation.");
                    }
                }
            }
            "harmony" | "harmonyos" => {
                println!("{}", "Generating HarmonyOS icons...".bold());
                match platform::harmony::generate_icons(
                    &context.project_root,
                    &icon_path,
                    background_color.as_deref(),
                    context.config.as_ref().and_then(|cfg| cfg.harmony.as_ref()),
                    foreground_path.as_deref(),
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping HarmonyOS icon generation.");
                    }
                }
            }
            "windows" | "win" => {
                println!("{}", "Generating Windows icon...".bold());
                match platform::windows::generate_icons(&context.project_root, &icon_path) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping Windows icon generation.");
                    }
                }
            }
            _ => {
                eprintln!(
                    "  {} Unknown platform: {}",
                    "Warning:".yellow(),
                    platform_name
                );
            }
        }
    }

    if generated_count > 0 {
        println!();
        println!("{}", "Icons generated successfully!".green().bold());
    } else {
        println!();
        println!("{}", "No icons were generated.".yellow());
    }

    Ok(())
}

/// Render `source` (SVG or PNG) to `output`, format chosen by the output
/// extension: `.ico` (multi-size Windows icon) or `.png` (single, `size` px).
/// No project context — used to (re)generate committed design assets.
fn write_converted_icon(
    source: &std::path::Path,
    output: &std::path::Path,
    size: Option<u32>,
) -> Result<()> {
    use crate::r#gen::icons;

    let src_is_svg = source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let out_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let bytes = match out_ext.as_str() {
        "ico" if src_is_svg => {
            let svg = std::fs::read_to_string(source)
                .with_context(|| format!("Failed to read {}", source.display()))?;
            icons::svg_to_ico_bytes(&svg, icons::WINDOWS_ICO_SIZES)?
        }
        "ico" => {
            let png = std::fs::read(source)
                .with_context(|| format!("Failed to read {}", source.display()))?;
            icons::png_to_ico_bytes(&png, icons::WINDOWS_ICO_SIZES)?
        }
        "png" if src_is_svg => {
            let svg = std::fs::read_to_string(source)
                .with_context(|| format!("Failed to read {}", source.display()))?;
            icons::svg_to_png_bytes(&svg, size.unwrap_or(1024))?
        }
        "png" => {
            let target = size.unwrap_or(1024);
            let img = image::open(source)
                .with_context(|| format!("Failed to read {}", source.display()))?
                .resize_exact(target, target, image::imageops::FilterType::Lanczos3);
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png)
                .context("Failed to encode PNG")?;
            buf.into_inner()
        }
        other => return Err(anyhow!("--output must end in .ico or .png (got '{other}')")),
    };

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, &bytes)
        .with_context(|| format!("Failed to write {}", output.display()))?;
    println!(
        "  {} {} -> {} ({} bytes)",
        "ok".green(),
        source.display(),
        output.display(),
        bytes.len()
    );
    Ok(())
}

/// `--check`: analyze the source exactly like the real pipeline would, render
/// every platform treatment (masks, tightening, safe zones), and write a
/// self-contained icon-preview.html for eyes-on review. Writes nothing else.
fn run_check_preview(
    source: &Path,
    foreground: Option<&Path>,
    background: Option<&str>,
) -> Result<()> {
    use base64::Engine as _;
    use image::{DynamicImage, RgbaImage, imageops};

    println!(
        "{}",
        "Check mode: preview only — no project files are written".bold()
    );
    println!();

    let img =
        image::open(source).with_context(|| format!("Failed to open {}", source.display()))?;
    let fg_img = foreground
        .map(|p| image::open(p).with_context(|| format!("Failed to open {}", p.display())))
        .transpose()?;

    // Same resolution ladder as the real generators — a preview that lies is
    // worse than none.
    let plan = appicon::resolve_layer_plan(&img, fg_img.as_ref(), background)?;
    for note in &plan.notes {
        println!("  {note}");
    }

    let bg = appicon::parse_hex_color(&plan.background)?;
    let compose = |canvas: u32, frac: f32| -> RgbaImage {
        let fg_layer = plan.render_foreground(canvas, frac);
        let mut base = RgbaImage::from_pixel(canvas, canvas, image::Rgba(bg));
        imageops::overlay(&mut base, &fg_layer, 0, 0);
        base
    };
    let to_b64 = |i: &RgbaImage| -> Result<String> {
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(i.clone())
            .write_to(&mut buf, image::ImageFormat::Png)
            .context("Failed to encode preview PNG")?;
        Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
    };

    // iOS: the OS applies the rounded mask; preview shows the raw tile under it.
    let ios = img
        .resize_exact(512, 512, imageops::FilterType::Lanczos3)
        .to_rgba8();
    // Android adaptive: derived foreground alone plus the composed layers.
    let android_fg = plan.render_foreground(432, appicon::ANDROID_SAFE_RADIUS_FRAC);
    let android = compose(432, appicon::ANDROID_SAFE_RADIUS_FRAC);
    // HarmonyOS layered icon composite.
    let harmony = compose(512, appicon::HARMONY_SAFE_RADIUS_FRAC);
    // macOS: reuse the exact production normalization (rounded-rect + dock ratio).
    let macos_tmp = platform::macos::build_macos_icon_source(source)?;
    let macos = image::open(macos_tmp.path())
        .context("Failed to reopen composed macOS icon")?
        .resize_exact(512, 512, imageops::FilterType::Lanczos3)
        .to_rgba8();
    // Windows: same tightening as the packed ICO, at the small sizes that matter.
    let win_master = crate::r#gen::icons::tighten_icon(img.to_rgba8());
    let win = |s: u32| -> RgbaImage {
        imageops::resize(&win_master, s, s, imageops::FilterType::Lanczos3)
    };

    let notes_html = plan
        .notes
        .iter()
        .map(|n| html_escape(n))
        .collect::<Vec<_>>()
        .join("</li><li>");

    let html = PREVIEW_TEMPLATE
        .replace("__SOURCE__", &html_escape(&source.display().to_string()))
        .replace("__BACKGROUND__", &plan.background)
        .replace("__NOTES__", &notes_html)
        .replace("__IOS__", &to_b64(&ios)?)
        .replace("__ANDROID_FG__", &to_b64(&android_fg)?)
        .replace("__ANDROID__", &to_b64(&android)?)
        .replace("__HARMONY__", &to_b64(&harmony)?)
        .replace("__MACOS__", &to_b64(&macos)?)
        .replace("__WIN48__", &to_b64(&win(48))?)
        .replace("__WIN32__", &to_b64(&win(32))?)
        .replace("__WIN16__", &to_b64(&win(16))?);

    let out = std::env::current_dir()?.join("icon-preview.html");
    std::fs::write(&out, html).with_context(|| format!("Failed to write {}", out.display()))?;
    println!();
    println!("  Preview: {}", out.display().to_string().cyan());
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&out).status();
    }
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const PREVIEW_TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Icon preview</title>
<style>
  body { margin: 0; padding: 40px 32px 72px; background: #14171a; color: #e8e8e5;
         font: 14px/1.6 -apple-system, "SF Pro Text", "PingFang SC", sans-serif; }
  .shell { max-width: 1080px; margin: 0 auto; }
  h1 { font-size: 20px; margin: 0 0 4px; }
  .src { color: #8a938f; font-size: 12.5px; font-family: Menlo, monospace; }
  .notes { margin: 20px 0 0; padding: 14px 18px 14px 34px; border: 1px solid #2c3236;
           border-radius: 8px; background: #191d20; color: #aab3ae; font-size: 13px; }
  h2 { font-size: 14px; margin: 40px 0 14px; padding-top: 22px; border-top: 1px solid #24292d;
       color: #c9cec9; }
  .row { display: flex; flex-wrap: wrap; gap: 28px; align-items: flex-end; }
  figure { margin: 0; text-align: center; }
  figcaption { margin-top: 8px; color: #8a938f; font-size: 12px; }
  .tile { display: inline-block; overflow: hidden; line-height: 0;
          box-shadow: 0 10px 28px rgb(0 0 0 / .35); }
  .tile img { display: block; width: 100%; height: 100%; }
  .alpha { background: repeating-conic-gradient(#22272b 0 25%, #171b1e 0 50%) 0 0 / 16px 16px; }
  .px img { image-rendering: pixelated; }
</style>
</head>
<body>
<div class="shell">
  <h1>Icon preview</h1>
  <div class="src">source: __SOURCE__ · background layer: __BACKGROUND__</div>
  <ul class="notes"><li>__NOTES__</li></ul>

  <h2>iOS — system applies the rounded mask</h2>
  <div class="row">
    <figure><span class="tile" style="width:180px;height:180px;border-radius:22.37%"><img src="data:image/png;base64,__IOS__"></span><figcaption>60pt @3x</figcaption></figure>
    <figure><span class="tile" style="width:120px;height:120px;border-radius:22.37%"><img src="data:image/png;base64,__IOS__"></span><figcaption>60pt @2x</figcaption></figure>
    <figure><span class="tile" style="width:60px;height:60px;border-radius:22.37%"><img src="data:image/png;base64,__IOS__"></span><figcaption>home screen</figcaption></figure>
    <figure><span class="tile" style="width:30px;height:30px;border-radius:22.37%"><img src="data:image/png;base64,__IOS__"></span><figcaption>spotlight</figcaption></figure>
  </div>

  <h2>Android adaptive — one composite, every launcher mask</h2>
  <div class="row">
    <figure><span class="tile" style="width:108px;height:108px;border-radius:50%"><img src="data:image/png;base64,__ANDROID__"></span><figcaption>circle</figcaption></figure>
    <figure><span class="tile" style="width:108px;height:108px;border-radius:28%"><img src="data:image/png;base64,__ANDROID__"></span><figcaption>squircle</figcaption></figure>
    <figure><span class="tile" style="width:108px;height:108px;border-radius:12%"><img src="data:image/png;base64,__ANDROID__"></span><figcaption>rounded square</figcaption></figure>
    <figure><span class="tile alpha" style="width:108px;height:108px"><img src="data:image/png;base64,__ANDROID_FG__"></span><figcaption>foreground layer</figcaption></figure>
  </div>

  <h2>HarmonyOS layered icon</h2>
  <div class="row">
    <figure><span class="tile" style="width:128px;height:128px;border-radius:25%"><img src="data:image/png;base64,__HARMONY__"></span><figcaption>composed</figcaption></figure>
    <figure><span class="tile" style="width:64px;height:64px;border-radius:25%"><img src="data:image/png;base64,__HARMONY__"></span><figcaption>64px</figcaption></figure>
  </div>

  <h2>macOS — production rounded-rect normalization</h2>
  <div class="row">
    <figure><span class="tile alpha" style="width:128px;height:128px"><img src="data:image/png;base64,__MACOS__"></span><figcaption>dock 128</figcaption></figure>
    <figure><span class="tile alpha" style="width:64px;height:64px"><img src="data:image/png;base64,__MACOS__"></span><figcaption>finder 64</figcaption></figure>
    <figure><span class="tile alpha" style="width:32px;height:32px"><img src="data:image/png;base64,__MACOS__"></span><figcaption>32</figcaption></figure>
  </div>

  <h2>Windows — tightened ICO cells (shown 2×, pixel-accurate)</h2>
  <div class="row px">
    <figure><span class="tile" style="width:96px;height:96px"><img src="data:image/png;base64,__WIN48__"></span><figcaption>48px taskbar</figcaption></figure>
    <figure><span class="tile" style="width:64px;height:64px"><img src="data:image/png;base64,__WIN32__"></span><figcaption>32px</figcaption></figure>
    <figure><span class="tile" style="width:32px;height:32px"><img src="data:image/png;base64,__WIN16__"></span><figcaption>16px title bar</figcaption></figure>
  </div>
</div>
</body>
</html>
"#;

struct IconCommandContext {
    project_root: PathBuf,
    config: Option<LingXiaConfig>,
    inferred_platform: Option<PlatformType>,
}

fn resolve_icon_context(current_dir: &std::path::Path) -> Result<IconCommandContext> {
    if has_host_config(current_dir) {
        let config = LingXiaConfig::load(current_dir).context(format!(
            "Failed to load {}. Are you in a LingXia project directory?",
            HOST_CONFIG_FILE
        ))?;
        return Ok(IconCommandContext {
            project_root: current_dir.to_path_buf(),
            config: Some(config),
            inferred_platform: None,
        });
    }

    if let Some(ctx) =
        platform::spm::find_apple_swift_package_context(current_dir, HOST_CONFIG_FILE)?
    {
        let config = LingXiaConfig::load(&ctx.host_project_root).context(format!(
            "Failed to load {} from the host project for this Apple Swift Package.",
            HOST_CONFIG_FILE
        ))?;
        return Ok(IconCommandContext {
            project_root: ctx.host_project_root,
            config: Some(config),
            inferred_platform: Some(ctx.inferred_platform),
        });
    }

    if let Some(host_root) =
        platform::detector::find_host_project_root(current_dir, HOST_CONFIG_FILE)
        && let Ok(inferred_platform) = platform::detector::detect_platform_type(current_dir)
    {
        let config = LingXiaConfig::load(&host_root).context(format!(
            "Failed to load {} from the detected host project.",
            HOST_CONFIG_FILE
        ))?;
        return Ok(IconCommandContext {
            project_root: host_root,
            config: Some(config),
            inferred_platform: Some(inferred_platform),
        });
    }

    if let Some(inferred_platform) =
        platform::spm::detect_local_apple_swift_package_platform(current_dir)?
    {
        return Ok(IconCommandContext {
            project_root: current_dir.to_path_buf(),
            config: None,
            inferred_platform: Some(inferred_platform),
        });
    }

    Err(anyhow!(
        "Failed to load {}. Are you in a LingXia project directory?\n\
         Supported icon targets without host config: local Apple Swift Packages (ios, macos).",
        HOST_CONFIG_FILE
    ))
}
