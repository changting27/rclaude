//! Analytics service matching services/analytics/.
//! Event tracking and telemetry.

use std::collections::HashMap;
use std::sync::Mutex;

/// Analytics event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalyticsEvent {
    pub name: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub timestamp: String,
}

/// Analytics sink trait.
pub trait AnalyticsSink: Send + Sync {
    fn log_event(&self, event: &AnalyticsEvent);
    fn flush(&self);
}

/// No-op sink (default).
#[allow(dead_code)]
struct NoopSink;
impl AnalyticsSink for NoopSink {
    fn log_event(&self, _event: &AnalyticsEvent) {}
    fn flush(&self) {}
}

static SINK: Mutex<Option<Box<dyn AnalyticsSink>>> = Mutex::new(None);
static EVENTS: Mutex<Option<Vec<AnalyticsEvent>>> = Mutex::new(None);

/// Attach an analytics sink.
pub fn attach_sink(sink: Box<dyn AnalyticsSink>) {
    *SINK.lock().unwrap() = Some(sink);
}

/// Log an analytics event.
pub fn log_event(name: &str, properties: HashMap<String, serde_json::Value>) {
    let event = AnalyticsEvent {
        name: name.to_string(),
        properties,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    if let Ok(sink) = SINK.lock() {
        if let Some(ref s) = *sink {
            s.log_event(&event);
        }
    }

    if let Ok(mut events) = EVENTS.lock() {
        events.get_or_insert_with(Vec::new).push(event);
    }
}

/// Get recorded events (for testing/debugging).
pub fn get_events() -> Vec<AnalyticsEvent> {
    EVENTS
        .lock()
        .ok()
        .and_then(|e| e.clone())
        .unwrap_or_default()
}

/// Flush all pending events.
pub fn flush() {
    if let Ok(sink) = SINK.lock() {
        if let Some(ref s) = *sink {
            s.flush();
        }
    }
}
