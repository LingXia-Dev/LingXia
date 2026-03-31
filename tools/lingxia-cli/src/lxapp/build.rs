use crate::lxapp::logic::{self, LogicBuildStatus};
use crate::lxapp::options::{BuildOptions, ProgressMode};
use crate::lxapp::package;
use crate::lxapp::project::Project;
use crate::lxapp::view::{self, ViewProgress};
use anyhow::{Result, anyhow};
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn run(args: &[String], cwd: &Path) -> Result<()> {
    let build_started = Instant::now();
    let options = BuildOptions::parse(args)?;
    let project = Project::discover(cwd, options.framework)?;

    if options.package && !options.release {
        return Err(anyhow!("--package requires --release"));
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
    println!(
        "  {} {}",
        style("Framework").dim(),
        project.framework.as_str()
    );
    println!();

    if project.output_dir.exists() {
        fs::remove_dir_all(&project.output_dir)?;
    }
    fs::create_dir_all(&project.output_dir)?;

    let reporter = Reporter::new(options.progress);
    reporter.start_parallel_tasks();
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
        let report = view::build(&view_project, &view_options, view_progress)?;
        Ok::<_, anyhow::Error>((report, started.elapsed()))
    });

    let (logic_report, logic_duration) = logic_handle
        .join()
        .map_err(|_| anyhow!("Logic build thread panicked"))??;
    let (view_report, view_duration) = view_handle
        .join()
        .map_err(|_| anyhow!("View build thread panicked"))??;

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

    if options.package {
        let package_started = Instant::now();
        let archive = package::package_dist(&project)?;
        reporter.package_built(&archive.display().to_string(), package_started.elapsed());
    }

    reporter.build_done(build_started.elapsed());

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
    #[allow(dead_code)]
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
            .set_message(format!("{} compiling", style("Logic").cyan()));
        self.view
            .set_message(format!("{} bundling", style("View").cyan()));
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
