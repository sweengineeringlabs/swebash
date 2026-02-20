/// Streaming test helpers for `AiEvent`.
///
/// Provides utilities for collecting events from a streaming chat receiver
/// and asserting properties of the event sequence.

use swebash_ai::api::types::AiEvent;
use tokio::sync::mpsc;

/// Drain all events from an `AiEvent` receiver.
///
/// Returns `(deltas, done_text)` where:
/// - `deltas` is the list of `Delta` content strings in order
/// - `done_text` is the content of the `Done` event, if received
///
/// The function returns once a `Done` or `Error` event is received or the channel closes.
pub async fn collect_stream_events(
    rx: &mut mpsc::Receiver<AiEvent>,
) -> (Vec<String>, Option<String>) {
    let mut deltas = Vec::new();
    let mut done_text = None;

    while let Some(event) = rx.recv().await {
        match event {
            AiEvent::Delta(d) => deltas.push(d),
            AiEvent::Done(d) => {
                done_text = Some(d);
                break;
            }
            AiEvent::ToolCall { .. } => {}
            AiEvent::Error(_) => break,
        }
    }

    (deltas, done_text)
}

/// Assert that the `Done` event text contains the expected substring.
///
/// # Panics
///
/// Panics if `done_text` is `None` or does not contain `expected`.
pub fn assert_done_event_contains(done_text: &Option<String>, expected: &str) {
    match done_text {
        Some(text) => {
            assert!(
                text.contains(expected),
                "Done event should contain '{expected}', got: '{text}'"
            );
        }
        None => panic!("Expected Done event, but none was received"),
    }
}

/// Assert that concatenated delta content equals the Done content (no duplication).
///
/// The invariant is: a consumer must print EITHER the deltas OR the Done text,
/// never both. If delta concat != done text, there is a duplication bug.
///
/// # Panics
///
/// Panics if the trimmed concatenation of deltas does not equal the trimmed done text.
pub fn assert_no_duplication(deltas: &[String], done_text: &str) {
    let delta_concat: String = deltas.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        delta_concat.trim(),
        done_text.trim(),
        "Concatenated deltas must equal Done content; printing both would duplicate the response"
    );
}

/// Assert that no events arrive after the Done event.
///
/// Waits briefly and then checks that the receiver yields no more events.
/// This verifies clean stream termination.
///
/// # Panics
///
/// Panics if any event is received within the timeout period.
pub async fn assert_no_events_after_done(rx: &mut mpsc::Receiver<AiEvent>) {
    let result = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await;
    match result {
        Ok(Some(event)) => panic!(
            "Expected no events after Done, but received: {event:?}"
        ),
        Ok(None) | Err(_) => {
            // Channel closed or timeout — both are correct.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn collect_stream_events_captures_deltas_and_done() {
        let (tx, mut rx) = mpsc::channel(16);
        tx.send(AiEvent::Delta("Hello ".into()))
            .await
            .unwrap();
        tx.send(AiEvent::Delta("world".into()))
            .await
            .unwrap();
        tx.send(AiEvent::Done("Hello world".into()))
            .await
            .unwrap();

        let (deltas, done) = collect_stream_events(&mut rx).await;
        assert_eq!(deltas, vec!["Hello ", "world"]);
        assert_eq!(done, Some("Hello world".into()));
    }

    #[tokio::test]
    async fn collect_stream_events_handles_no_done() {
        let (tx, mut rx) = mpsc::channel(16);
        tx.send(AiEvent::Delta("partial".into()))
            .await
            .unwrap();
        drop(tx); // Close channel without sending Done.

        let (deltas, done) = collect_stream_events(&mut rx).await;
        assert_eq!(deltas, vec!["partial"]);
        assert_eq!(done, None);
    }

    #[test]
    fn assert_done_event_contains_passes_on_match() {
        let done = Some("Hello from the mock".into());
        assert_done_event_contains(&done, "mock");
    }

    #[test]
    #[should_panic(expected = "Expected Done event")]
    fn assert_done_event_contains_panics_on_none() {
        assert_done_event_contains(&None, "anything");
    }

    #[test]
    #[should_panic(expected = "Done event should contain")]
    fn assert_done_event_contains_panics_on_mismatch() {
        let done = Some("actual content".into());
        assert_done_event_contains(&done, "missing");
    }

    #[test]
    fn assert_no_duplication_passes_when_equal() {
        let deltas = vec!["Hello ".into(), "world".into()];
        assert_no_duplication(&deltas, "Hello world");
    }

    #[test]
    #[should_panic(expected = "Concatenated deltas must equal Done content")]
    fn assert_no_duplication_panics_on_mismatch() {
        let deltas = vec!["Hello".into()];
        assert_no_duplication(&deltas, "different");
    }

    #[tokio::test]
    async fn assert_no_events_after_done_passes_on_closed_channel() {
        let (_tx, mut rx) = mpsc::channel::<AiEvent>(16);
        drop(_tx);
        assert_no_events_after_done(&mut rx).await;
    }

    #[tokio::test]
    async fn assert_no_events_after_done_passes_on_timeout() {
        let (_tx, mut rx) = mpsc::channel::<AiEvent>(16);
        // Keep tx alive but send nothing.
        assert_no_events_after_done(&mut rx).await;
    }
}
