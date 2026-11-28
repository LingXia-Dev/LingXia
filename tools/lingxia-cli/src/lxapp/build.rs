use crate::lxapp::logic::{self, LogicBuildStatus};
use crate::lxapp::options::{BuildOptions, ProgressMode};
use crate::lxapp::package;
use crate::lxapp::project::Project;
use crate::lxapp::typecheck;
use crate::lxapp::view::{self, ViewProgress};
use anyhow::{Result, anyhow};
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn run(args: &[String], cwd: &Path) -> Result<()> {
    run_with_context(args, cwd, false, None)
}

/// A dev session's build; `output` replaces the project's `dist` when given.
pub fn run_for_dev(args: &[String], cwd: &Path, output: Option<&Path>) -> Result<()> {
    run_with_context(args, cwd, true, output)
}

fn run_with_context(
    args: &[String],
    cwd: &Path,
    dev_session: bool,
    output: Option<&Path>,
) -> Result<()> {
    let _preparation = crate::build_prepare::Scope::enter();
    let _ts_session = super::ts_worker::keep_alive();
    let build_started = Instant::now();
    let mut options = BuildOptions::parse(args)?;
    options.dev_session = dev_session;
    let mut project = Project::discover(cwd)?;
    if let Some(output) = output {
        project.output_dir = output.to_path_buf();
        options.inputs_record = Some(super::session_inputs_record(output));
    }

    if options.package && !options.release {
        return Err(anyhow!("--package requires --release"));
    }

    // A dev session prepared once at start; its companion keeps outputs live.
    if !dev_session {
        crate::build_prepare::prepare(&project.root)?;
    }

    println!();
    println!(
        "  {} {}",
        style("LingXia Build").bold().cyan(),
        if options.release {
            style("(release)").yellow()
        } else {
            style("(debug)").dim()
        }
    );
    println!("  {} {}", style("Project").dim(), project.root.display());
    println!(
        "  {} {}",
        style("Output").dim(),
        project.output_dir.display()
    );
    // A GPU lxapp has no WebView framework; the one `discover` falls back to is not news.
    let framework = if crate::lxapp::framework::is_gpu_lxapp(&project.root) {
        "gpu"
    } else {
        project.framework.as_str()
    };
    println!("  {} {}", style("Framework").dim(), framework);
    println!();

    // Before either half builds: the page names are what a Logic author's
    // editor completes against, so they must exist even if the build then fails.
    crate::lxapp::generate_types(&project)?;

    let reporter = Reporter::new(options.progress);
    build_staged(&project.output_dir, |staging| {
        let staged = Project {
            output_dir: staging.to_path_buf(),
            ..project.clone()
        };
        build_output(&staged, &options, &reporter)
    })?;

    if options.package {
        let package_started = Instant::now();
        let archive = package::package_dist(&project)?;
        reporter.package_built(&archive.display().to_string(), package_started.elapsed());
    }

    reporter.build_done(build_started.elapsed());

    Ok(())
}

/// `lingxia check`: everything a build refuses and every type error, in one report; the
/// output is built aside and thrown away.
/// `host_problems` come from the host project around the lxapp, reported with its own.
pub fn check(cwd: &Path, host_problems: Vec<String>) -> Result<()> {
    let _preparation = crate::build_prepare::Scope::enter();
    let _ts_session = super::ts_worker::keep_alive();
    let project = Project::discover(cwd)?;
    crate::build_prepare::prepare(&project.root)?;
    println!();
    println!(
        "  {} {}",
        style("LingXia Check").bold().cyan(),
        style(project.root.display()).dim()
    );
    // Wiring that `tsc -b` cannot follow makes its errors misleading: report it instead of them.
    let wiring = crate::lxapp::generate_types(&project);
    let options = BuildOptions {
        dev_session: false,
        release: false,
        package: false,
        typecheck: wiring.is_ok(),
        progress: ProgressMode::Plain,
        inputs_record: None,
        messages: None,
    };
    let staging = sibling(&project.output_dir, "checking");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let checked = Project {
        output_dir: staging.clone(),
        ..project.clone()
    };
    let built = build_output(&checked, &options, &Reporter::new(ProgressMode::Plain));
    let _ = fs::remove_dir_all(&staging);
    let failures: Vec<String> = host_problems
        .into_iter()
        .chain(
            [wiring.err(), built.err()]
                .into_iter()
                .flatten()
                .map(|error| format!("{error:#}")),
        )
        .collect();
    if !failures.is_empty() {
        return Err(anyhow!(failures.join("\n\n")));
    }
    println!("  {} no problems", style("✓").green());
    Ok(())
}

/// Build into a staging directory and swap it in whole: a failed build leaves the last good
/// output in place, which a dev session keeps serving.
fn build_staged(output_dir: &Path, build: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let staging = staging_dir(output_dir);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    if let Err(err) = build(&staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }
    replace_dir(&staging, output_dir)
}

/// `dist` builds into `.dist.building-<pid>` next to it: the same depth keeps relative paths
/// valid, and the pid keeps dev sessions of other platforms, which rebuild the same lxapp on
/// the same save, out of each other's staging.
fn staging_dir(output_dir: &Path) -> std::path::PathBuf {
    sibling(output_dir, "building")
}

fn sibling(output_dir: &Path, role: &str) -> std::path::PathBuf {
    let name = output_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dist".to_string());
    output_dir.with_file_name(format!(".{name}.{role}-{}", std::process::id()))
}

/// Swap `staging` in as `output_dir`; the old output is gone only once the new one is in place.
fn replace_dir(staging: &Path, output_dir: &Path) -> Result<()> {
    let previous = sibling(output_dir, "previous");
    if previous.exists() {
        fs::remove_dir_all(&previous)?;
    }
    if output_dir.exists() {
        fs::rename(output_dir, &previous)?;
    }
    fs::rename(staging, output_dir)?;
    if previous.exists() {
        fs::remove_dir_all(&previous)?;
    }
    Ok(())
}

fn build_output(project: &Project, options: &BuildOptions, reporter: &Reporter) -> Result<()> {
    let messages = super::i18n::compile(&project.root, &project.output_dir, options.dev_session)?;
    let options = &BuildOptions {
        messages,
        ..options.clone()
    };
    reporter.start_parallel_tasks();
    let install_duration_hint = view::prepare_tooling(project, options, reporter.view_progress())?;
    let logic_progress = reporter.logic_progress();
    let view_progress = reporter.view_progress();
    let logic_project = project.clone();
    let logic_options = options.clone();
    let view_project = project.clone();
    let view_options = options.clone();

    let logic_handle = std::thread::spawn(move || {
        let started = Instant::now();
        let report = logic::build(&logic_project, &logic_options, logic_progress)?;
        Ok::<_, anyhow::Error>((report, started.elapsed()))
    });
    let view_handle = std::thread::spawn(move || {
        let started = Instant::now();
        let report = view::build(
            &view_project,
            &view_options,
            view_progress,
            install_duration_hint,
        )?;
        Ok::<_, anyhow::Error>((report, started.elapsed()))
    });

    // A dev session reports type errors from its watcher instead, without holding the swap.
    let types_handle = (options.typecheck
        && !options.dev_session
        && typecheck::applies(&project.root))
    .then(|| {
        let root = project.root.clone();
        std::thread::spawn(move || typecheck::run(&root))
    });

    let logic = logic_handle
        .join()
        .map_err(|_| anyhow!("Logic build thread panicked"))?;
    let view = view_handle
        .join()
        .map_err(|_| anyhow!("View build thread panicked"))?;
    let types = types_handle
        .map(|handle| {
            handle
                .join()
                .map_err(|_| anyhow!("Type check thread panicked"))
        })
        .transpose()?;
    // Every half's problems in one failure, so one run lists all of them.
    let mut failures = Vec::new();
    if let Err(error) = &logic {
        failures.push(format!("{error:#}"));
    }
    if let Err(error) = &view {
        failures.push(format!("{error:#}"));
    }
    match &types {
        Some(Ok(problems)) if !problems.is_empty() => failures.push(typecheck::report(problems)),
        Some(Err(error)) => failures.push(format!("{error:#}")),
        _ => {}
    }
    if !failures.is_empty() {
        return Err(anyhow!(failures.join("\n\n")));
    }
    let (logic_report, logic_duration) = logic?;
    let (view_report, view_duration) = view?;

    match logic_report.status {
        LogicBuildStatus::Built { output_path } => {
            reporter.logic_built(&output_path.display().to_string(), logic_duration);
        }
        LogicBuildStatus::Disabled => {
            reporter.logic_disabled(logic_duration);
        }
        LogicBuildStatus::Skipped => {
            reporter.logic_skipped(logic_duration);
        }
    }

    reporter.view_summary(
        view_report.page_count,
        view_report.framework.as_str(),
        view_report.install_duration,
        view_report.prepare_duration,
        view_report.bundle_duration,
        view_report.finalize_duration,
        view_duration,
    );

    crate::lxapp::stamp_output_html(&project.output_dir)?;
    if options.dev_session {
        super::i18n::declare_pseudo(&project.output_dir)?;
    }

    if options.release {
        crate::lxapp::hardening::harden_release_output(project)?;
    }
    Ok(())
}

enum Reporter {
    Task(TaskReporter),
    Plain,
}

impl Reporter {
    fn new(mode: ProgressMode) -> Self {
        match mode {
            ProgressMode::Task => Self::Task(TaskReporter::new()),
            ProgressMode::Plain => Self::Plain,
        }
    }

    fn start_parallel_tasks(&self) {
        match self {
            Self::Task(reporter) => reporter.start(),
            Self::Plain => println!("  {} running logic and view in parallel", style("▸").dim()),
        }
    }

    fn logic_progress(&self) -> Option<ProgressBar> {
        match self {
            Self::Task(reporter) => Some(reporter.logic.clone()),
            Self::Plain => None,
        }
    }

    fn view_progress(&self) -> Option<ViewProgress> {
        match self {
            Self::Task(reporter) => Some(ViewProgress::new(reporter.view.clone())),
            Self::Plain => None,
        }
    }

    fn logic_built(&self, output_path: &str, duration: Duration) {
        let msg = format!(
            "{} built → {} {}",
            style("Logic").cyan(),
            output_path,
            style(format_duration(duration)).dim()
        );
        match self {
            Self::Task(reporter) => reporter.logic_done(&msg),
            Self::Plain => println!("  {} {}", style("✓").green(), msg),
        }
    }

    fn logic_disabled(&self, duration: Duration) {
        let msg = format!(
            "{} disabled {}",
            style("Logic").cyan(),
            style(format_duration(duration)).dim()
        );
        match self {
            Self::Task(reporter) => reporter.logic_done(&msg),
            Self::Plain => println!("  {} {}", style("–").dim(), msg),
        }
    }

    fn logic_skipped(&self, duration: Duration) {
        let msg = format!(
            "{} skipped {}",
            style("Logic").cyan(),
            style(format_duration(duration)).dim()
        );
        match self {
            Self::Task(reporter) => reporter.logic_done(&msg),
            Self::Plain => println!("  {} {}", style("–").dim(), msg),
        }
    }

    fn view_summary(
        &self,
        page_count: usize,
        framework: &str,
        install_duration: Option<Duration>,
        prepare_duration: Duration,
        bundle_duration: Duration,
        finalize_duration: Duration,
        total_duration: Duration,
    ) {
        let mut parts = Vec::new();
        if let Some(d) = install_duration
            && !d.is_zero()
        {
            parts.push(format!("deps {}", format_duration(d)));
        }
        parts.push(format!("prepare {}", format_duration(prepare_duration)));
        if !bundle_duration.is_zero() {
            parts.push(format!("bundle {}", format_duration(bundle_duration)));
        }
        if !finalize_duration.is_zero() {
            parts.push(format!("finalize {}", format_duration(finalize_duration)));
        }
        let breakdown = style(format!("({})", parts.join(" · "))).dim();
        let msg = format!(
            "{} {} {} pages {} {}",
            style("View").cyan(),
            page_count,
            framework,
            style(format_duration(total_duration)).dim(),
            breakdown
        );
        match self {
            Self::Task(reporter) => reporter.view_done(&msg),
            Self::Plain => println!("  {} {}", style("✓").green(), msg),
        }
    }

    fn package_built(&self, archive_path: &str, duration: Duration) {
        let msg = format!(
            "{} → {} {}",
            style("Package").cyan(),
            archive_path,
            style(format_duration(duration)).dim()
        );
        match self {
            Self::Task(reporter) => reporter.package_done(&msg),
            Self::Plain => println!("  {} {}", style("✓").green(), msg),
        }
    }

    fn build_done(&self, duration: Duration) {
        let msg = format!(
            "Build done in {}",
            style(format_duration(duration)).green().bold()
        );
        match self {
            Self::Task(reporter) => reporter.finish(&msg),
            Self::Plain => {
                println!();
                println!("  {} {}", style("✓").green().bold(), style(msg).bold());
            }
        }
    }
}

struct TaskReporter {
    multi: MultiProgress,
    build: ProgressBar,
    logic: ProgressBar,
    view: ProgressBar,
}

impl TaskReporter {
    fn new() -> Self {
        let multi = MultiProgress::new();
        let spinner_style = ProgressStyle::with_template("  {spinner} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "]);
        let build = multi.add(ProgressBar::new_spinner());
        build.set_style(spinner_style.clone());
        let logic = multi.add(ProgressBar::new_spinner());
        logic.set_style(spinner_style.clone());
        let view = multi.add(ProgressBar::new_spinner());
        view.set_style(spinner_style);
        Self {
            multi,
            build,
            logic,
            view,
        }
    }

    fn start(&self) {
        self.build
            .set_message(format!("{}", style("Building...").bold()));
        self.logic
            .set_message(format!("{} waiting", style("Logic").cyan()));
        self.view
            .set_message(format!("{} waiting", style("View").cyan()));
        let _ = &self.multi;
        self.build.enable_steady_tick(Duration::from_millis(80));
        self.logic.enable_steady_tick(Duration::from_millis(80));
        self.view.enable_steady_tick(Duration::from_millis(80));
    }

    fn logic_done(&self, message: &str) {
        self.logic
            .finish_with_message(format!("{} {}", style("✓").green(), message));
    }

    fn view_done(&self, message: &str) {
        self.view
            .finish_with_message(format!("{} {}", style("✓").green(), message));
    }

    fn package_done(&self, message: &str) {
        self.build
            .set_message(format!("{} {}", style("✓").green(), message));
    }

    fn finish(&self, message: &str) {
        self.build.finish_with_message(format!(
            "{} {}",
            style("✓").green().bold(),
            style(message).bold()
        ));
    }
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs_f64() >= 10.0 {
        format!("{:.1}s", duration.as_secs_f64())
    } else if duration.as_millis() >= 1000 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

#[cfg(test)]
mod staging_tests {
    use super::*;

    #[test]
    fn a_failed_build_leaves_the_last_output_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let dist = temp.path().join("dist");
        fs::create_dir_all(dist.join("pages/a")).unwrap();
        fs::write(dist.join("pages/a/index.js"), "old a").unwrap();
        fs::create_dir_all(dist.join("pages/b")).unwrap();
        fs::write(dist.join("pages/b/index.js"), "old b").unwrap();

        // Page a compiles, page b fails: nothing of the attempt reaches `dist`.
        let result = build_staged(&dist, |staging| {
            fs::create_dir_all(staging.join("pages/a")).unwrap();
            fs::write(staging.join("pages/a/index.js"), "new a").unwrap();
            Err(anyhow!("pages/b/index.tsx: 1 authoring problem"))
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(dist.join("pages/a/index.js")).unwrap(),
            "old a"
        );
        assert_eq!(
            fs::read_to_string(dist.join("pages/b/index.js")).unwrap(),
            "old b"
        );
        assert!(!staging_dir(&dist).exists());

        build_staged(&dist, |staging| {
            fs::create_dir_all(staging.join("pages/a")).unwrap();
            fs::write(staging.join("pages/a/index.js"), "new a").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(dist.join("pages/a/index.js")).unwrap(),
            "new a"
        );
        assert!(!dist.join("pages/b").exists());
        let siblings: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(siblings, vec![std::ffi::OsString::from("dist")]);
    }
}
