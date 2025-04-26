use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub fn setup() -> tracing_appender::non_blocking::WorkerGuard {
    // Configure file logging
    let file_appender = tracing_appender::rolling::daily("logs", "app.log"); // Log to logs/app.log.YYYY-MM-DD
    let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_appender);

    // Configure console logging
    let console_layer = fmt::layer().with_writer(std::io::stdout);

    // Configure file logging layer
    let file_layer = fmt::layer()
        .with_writer(non_blocking_writer)
        .with_ansi(false); // Don't write ANSI colors to files

    // Build the subscriber
    tracing_subscriber::registry()
        .with(
            // Default level info, but allow overriding with RUST_LOG env var
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .with(console_layer)
        .with(file_layer)
        // If using tracing-log bridge:
        // .with(tracing_log::LogTracer::builder().init().unwrap()) // Check if needed
        .init(); // Set as global subscriber

    // Keep the guard (_guard) in scope - dropping it stops the background writer.
    // If setting up tracing in `main`, you can just leak the guard:
    guard
}
