//! Shared tracing initialization for project binaries.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt::writer::BoxMakeWriter};

/// Keeps the non-blocking tracing worker alive for the process lifetime.
pub(crate) struct TracingGuard {
    _worker_guard: WorkerGuard,
}

/// Initializes process-wide tracing with a non-blocking stderr writer.
pub(crate) fn init_tracing(default_filter: &str) -> TracingGuard {
    let (writer, worker_guard) = tracing_appender::non_blocking(std::io::stderr());
    let writer = BoxMakeWriter::new(writer);

    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter.into()),
        )
        .try_init();

    TracingGuard {
        _worker_guard: worker_guard,
    }
}
