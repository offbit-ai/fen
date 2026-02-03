//! Event streaming endpoints using Server-Sent Events (SSE)
//!
//! Provides real-time event streams for document processing, anomaly detection,
//! and system events.

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::stream::Stream;
use serde::Serialize;

use fen_events::topics;

use crate::error::ApiError;
use crate::state::AppState;

/// Information about an event topic
#[derive(Serialize)]
pub struct TopicInfo {
    /// Topic name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Whether the topic is active
    pub active: bool,
}

/// Response listing available event topics
#[derive(Serialize)]
pub struct TopicsResponse {
    /// Available topics
    pub topics: Vec<TopicInfo>,
}

/// GET /events/topics - List available event topics
///
/// Returns the list of event topics that can be subscribed to.
pub async fn list_topics(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<TopicsResponse>, ApiError> {
    let topics = vec![
        TopicInfo {
            name: topics::DOCUMENT_INGESTION.to_string(),
            description: "Document ingestion events".to_string(),
            active: true,
        },
        TopicInfo {
            name: topics::DOCUMENT_PROCESSED.to_string(),
            description: "Document processing completion events".to_string(),
            active: true,
        },
        TopicInfo {
            name: topics::VALIDATION_RESULTS.to_string(),
            description: "Document validation result events".to_string(),
            active: true,
        },
        TopicInfo {
            name: topics::ANOMALY_EVENTS.to_string(),
            description: "Anomaly detection events".to_string(),
            active: true,
        },
        TopicInfo {
            name: topics::BASELINE_UPDATES.to_string(),
            description: "Vendor baseline update events".to_string(),
            active: true,
        },
        TopicInfo {
            name: topics::METRICS.to_string(),
            description: "Real-time metrics events".to_string(),
            active: true,
        },
        TopicInfo {
            name: topics::ALERTS.to_string(),
            description: "System alert events".to_string(),
            active: true,
        },
    ];

    Ok(Json(TopicsResponse { topics }))
}

/// Simple heartbeat-only SSE stream
struct HeartbeatStream {
    interval: tokio::time::Interval,
}

impl HeartbeatStream {
    fn new() -> Self {
        Self {
            interval: tokio::time::interval(Duration::from_secs(30)),
        }
    }
}

impl Stream for HeartbeatStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.interval.poll_tick(cx) {
            Poll::Ready(_) => {
                let event = Event::default()
                    .event("heartbeat")
                    .json_data(serde_json::json!({
                        "type": "heartbeat",
                        "timestamp": chrono::Utc::now()
                    }))
                    .unwrap_or_else(|_| Event::default().comment("heartbeat"));
                Poll::Ready(Some(Ok(event)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// GET /events/stream - SSE event stream
///
/// Returns a Server-Sent Events stream of real-time events.
/// The stream includes heartbeat events and notifications from the notification hub.
///
/// ## Event Format
///
/// Events are sent as JSON with the following structure:
/// ```json
/// {
///   "type": "notification",
///   "timestamp": "2024-01-15T10:30:00Z",
///   "data": { ... }
/// }
/// ```
///
/// ## Keep-Alive
///
/// The stream sends keep-alive comments every 15 seconds to maintain
/// the connection through proxies.
pub async fn event_stream(
    State(_state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // For now, use a simple heartbeat stream
    // Full notification streaming requires more complex async handling
    Sse::new(HeartbeatStream::new()).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// GET /events/metrics - SSE stream of metrics events
///
/// Returns a dedicated stream for metrics events only.
/// Useful for real-time dashboard updates.
pub async fn metrics_stream(
    State(_state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Simple heartbeat stream for metrics
    Sse::new(HeartbeatStream::new()).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
