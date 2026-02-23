/// Tracing event capture and assertion utilities for observability tests.
///
/// Provides `TracingCapture`, an RAII guard that installs a thread-local
/// tracing subscriber to capture events, and helper methods for asserting
/// that specific log/trace events were (or were not) emitted.

use std::sync::Arc;

use parking_lot::Mutex;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

use crate::error::TestError;

/// A captured tracing event.
#[derive(Debug, Clone)]
pub struct CapturedEvent {
    /// The severity level of the event.
    pub level: Level,
    /// The target module path (e.g., `swebash_llm::core`).
    pub target: String,
    /// The formatted message (from the `message` field).
    pub message: String,
}

/// RAII guard that captures tracing events for the current thread.
///
/// Uses `tracing::subscriber::set_default` so only the current thread is
/// affected — safe for parallel test execution.
///
/// # Example
///
/// ```
/// use swebash_test::observe::TracingCapture;
/// use tracing::Level;
///
/// let capture = TracingCapture::install();
/// tracing::info!("hello world");
/// capture.assert_event_emitted(Level::INFO, "hello");
/// ```
pub struct TracingCapture {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
    _guard: tracing::subscriber::DefaultGuard,
}

impl TracingCapture {
    /// Install a capturing subscriber on the current thread.
    ///
    /// Returns an RAII guard; the subscriber is removed when the guard is dropped.
    pub fn install() -> Self {
        let events: Arc<Mutex<Vec<CapturedEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let layer = CaptureLayer {
            events: Arc::clone(&events),
        };
        let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
        let guard = tracing::subscriber::set_default(subscriber);
        Self {
            events,
            _guard: guard,
        }
    }

    /// All captured events so far.
    pub fn events(&self) -> Vec<CapturedEvent> {
        self.events.lock().clone()
    }

    /// Captured events filtered to a specific level.
    pub fn events_at_level(&self, level: Level) -> Vec<CapturedEvent> {
        self.events
            .lock()
            .iter()
            .filter(|e| e.level == level)
            .cloned()
            .collect()
    }

    /// Captured events whose message contains the given substring.
    pub fn events_containing(&self, substring: &str) -> Vec<CapturedEvent> {
        self.events
            .lock()
            .iter()
            .filter(|e| e.message.contains(substring))
            .cloned()
            .collect()
    }

    /// Assert that at least one event at `level` with a message containing
    /// `substring` was captured.
    ///
    /// # Panics
    ///
    /// Panics if no matching event is found.
    pub fn assert_event_emitted(&self, level: Level, substring: &str) {
        let events = self.events.lock();
        let found = events
            .iter()
            .any(|e| e.level == level && e.message.contains(substring));
        assert!(
            found,
            "Expected tracing event at {level} containing '{substring}', \
             captured {} events: {:?}",
            events.len(),
            events
                .iter()
                .map(|e| format!("[{}] {}", e.level, e.message))
                .collect::<Vec<_>>()
        );
    }

    /// Assert that no events were captured at the given level.
    ///
    /// # Panics
    ///
    /// Panics if any events at `level` exist.
    pub fn assert_no_events_at_level(&self, level: Level) {
        let at_level: Vec<_> = self
            .events
            .lock()
            .iter()
            .filter(|e| e.level == level)
            .cloned()
            .collect();
        assert!(
            at_level.is_empty(),
            "Expected no events at {level}, but found {}: {:?}",
            at_level.len(),
            at_level
                .iter()
                .map(|e| &e.message)
                .collect::<Vec<_>>()
        );
    }

    /// Return a `TestError::Observability` if an expected event was not emitted.
    ///
    /// Non-panicking alternative to `assert_event_emitted`.
    pub fn expect_event(
        &self,
        level: Level,
        substring: &str,
    ) -> Result<(), TestError> {
        let events = self.events.lock();
        let found = events
            .iter()
            .any(|e| e.level == level && e.message.contains(substring));
        if found {
            Ok(())
        } else {
            Err(TestError::Observability(format!(
                "no event at {level} containing '{substring}'"
            )))
        }
    }
}

// ── Internal: CaptureLayer ──────────────────────────────────────────

struct CaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        self.events.lock().push(CapturedEvent {
            level: *metadata.level(),
            target: metadata.target().to_string(),
            message: visitor.message,
        });
    }
}

// ── Internal: MessageVisitor ────────────────────────────────────────

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_info_event() {
        let capture = TracingCapture::install();
        tracing::info!("test info message");
        let events = capture.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level, Level::INFO);
        assert!(events[0].message.contains("test info message"));
    }

    #[test]
    fn capture_multiple_levels() {
        let capture = TracingCapture::install();
        tracing::debug!("debug msg");
        tracing::warn!("warn msg");
        tracing::error!("error msg");
        assert_eq!(capture.events().len(), 3);
    }

    #[test]
    fn events_at_level_filters() {
        let capture = TracingCapture::install();
        tracing::info!("info one");
        tracing::warn!("warn one");
        tracing::info!("info two");
        let infos = capture.events_at_level(Level::INFO);
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn events_containing_filters() {
        let capture = TracingCapture::install();
        tracing::info!("alpha event");
        tracing::info!("beta event");
        tracing::info!("alpha again");
        let alphas = capture.events_containing("alpha");
        assert_eq!(alphas.len(), 2);
    }

    #[test]
    fn assert_event_emitted_passes() {
        let capture = TracingCapture::install();
        tracing::error!("something broke");
        capture.assert_event_emitted(Level::ERROR, "broke");
    }

    #[test]
    #[should_panic(expected = "Expected tracing event")]
    fn assert_event_emitted_fails() {
        let capture = TracingCapture::install();
        tracing::info!("only info");
        capture.assert_event_emitted(Level::ERROR, "missing");
    }

    #[test]
    fn assert_no_events_at_level_passes() {
        let capture = TracingCapture::install();
        tracing::info!("info only");
        capture.assert_no_events_at_level(Level::ERROR);
    }

    #[test]
    #[should_panic(expected = "Expected no events")]
    fn assert_no_events_at_level_fails() {
        let capture = TracingCapture::install();
        tracing::error!("oops");
        capture.assert_no_events_at_level(Level::ERROR);
    }

    #[test]
    fn expect_event_returns_ok() {
        let capture = TracingCapture::install();
        tracing::warn!("careful now");
        assert!(capture.expect_event(Level::WARN, "careful").is_ok());
    }

    #[test]
    fn expect_event_returns_err() {
        let capture = TracingCapture::install();
        tracing::info!("only info");
        let result = capture.expect_event(Level::ERROR, "missing");
        match result {
            Err(TestError::Observability(msg)) => {
                assert!(msg.contains("missing"));
            }
            other => panic!("Expected Observability error, got: {other:?}"),
        }
    }
}
