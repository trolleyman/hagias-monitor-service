use tracing::{debug, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer as _, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

pub fn setup() -> tracing_appender::non_blocking::WorkerGuard {
    // Configure file logging
    let root_directory = std::env::current_exe()
        .ok()
        .and_then(|f| f.parent().map(|p| p.to_owned()))
        .unwrap_or(".".into());
    let log_directory = root_directory.join("logs");
    let file_appender = tracing_appender::rolling::daily(&log_directory, "app.log"); // Log to logs/app.log.YYYY-MM-DD
    let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_appender);

    // Configure console logging with simple format and info+ level
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_thread_names(false)
        .with_span_events(fmt::format::FmtSpan::NONE)
        .with_level(true)
        .with_timer(ConsoleTimeFormat)
        .with_writer(std::io::stdout)
        .with_filter(LevelFilter::INFO);

    // Configure file logging layer with detailed format and debug+ level
    let file_layer = fmt::layer()
        // .event_format(Format::<Full, SystemTime>::default())
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_span_events(fmt::format::FmtSpan::ENTER | fmt::format::FmtSpan::EXIT)
        .with_level(true)
        .with_timer(FileTimeFormat)
        .with_writer(non_blocking_writer)
        .with_filter(LevelFilter::DEBUG);

    // Build the subscriber with different filters for console and file
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        // If using tracing-log bridge:
        // .with(tracing_log::LogTracer::builder().init().unwrap()) // Check if needed
        .init(); // Set as global subscriber

    debug!("Logging initialized in {}", log_directory.display());
    debug!(
        "Current directory: {}",
        std::env::current_dir().unwrap_or(".".into()).display()
    );

    // Keep the guard (_guard) in scope - dropping it stops the background writer.
    // If setting up tracing in `main`, you can just leak the guard:
    guard
}

struct ConsoleTimeFormat;

impl tracing_subscriber::fmt::time::FormatTime for ConsoleTimeFormat {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let time = jiff::Zoned::now();
        write!(
            w,
            "{}:{}:{}.{}",
            time.hour(),
            time.minute(),
            time.second(),
            time.millisecond()
        )
    }
}

struct FileTimeFormat;

impl tracing_subscriber::fmt::time::FormatTime for FileTimeFormat {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let time = jiff::Zoned::now();
        write!(
            w,
            "{}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}[{}]",
            time.year(),
            time.month(),
            time.day(),
            time.hour(),
            time.minute(),
            time.second(),
            time.subsec_nanosecond(),
            time.offset()
        )
    }
}
