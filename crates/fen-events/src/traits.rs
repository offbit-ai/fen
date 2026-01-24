//! Event producer and consumer traits.
//!
//! These traits abstract over the underlying event transport (Kafka, in-memory, etc.)
//! allowing for easy testing and different deployment configurations.

use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during event operations.
#[derive(Debug, Error)]
pub enum EventError {
    /// Failed to publish an event.
    #[error("Failed to publish event: {0}")]
    Publish(String),

    /// Failed to consume events.
    #[error("Failed to consume events: {0}")]
    Consume(String),

    /// Failed to commit offsets.
    #[error("Failed to commit offsets: {0}")]
    Commit(String),

    /// Failed to subscribe to topics.
    #[error("Failed to subscribe to topics: {0}")]
    Subscribe(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),

    /// Timeout error.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// A raw event as received from the transport.
#[derive(Debug, Clone)]
pub struct RawEvent {
    /// Topic the event was received from.
    pub topic: String,
    /// Partition within the topic.
    pub partition: i32,
    /// Offset within the partition.
    pub offset: i64,
    /// Event key (used for partitioning).
    pub key: Option<Vec<u8>>,
    /// Event payload.
    pub payload: Vec<u8>,
    /// Event headers/metadata.
    pub headers: HashMap<String, String>,
    /// Timestamp when the event was produced.
    pub timestamp: Option<i64>,
}

impl RawEvent {
    /// Get the key as a UTF-8 string if present.
    pub fn key_str(&self) -> Option<&str> {
        self.key.as_ref().and_then(|k| std::str::from_utf8(k).ok())
    }

    /// Get a header value.
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(|s| s.as_str())
    }
}

/// Trait for event producers.
///
/// Implementations publish events to a message transport (Kafka, in-memory queue, etc.)
#[async_trait]
pub trait EventProducer: Send + Sync {
    /// Publish a single event.
    ///
    /// # Arguments
    /// * `topic` - The topic to publish to
    /// * `key` - The partition key (events with same key go to same partition)
    /// * `payload` - The serialized event payload
    async fn publish(&self, topic: &str, key: &[u8], payload: &[u8]) -> Result<(), EventError>;

    /// Publish a single event with headers.
    ///
    /// # Arguments
    /// * `topic` - The topic to publish to
    /// * `key` - The partition key
    /// * `payload` - The serialized event payload
    /// * `headers` - Additional metadata headers
    async fn publish_with_headers(
        &self,
        topic: &str,
        key: &[u8],
        payload: &[u8],
        headers: HashMap<String, String>,
    ) -> Result<(), EventError> {
        // Default implementation ignores headers
        let _ = headers;
        self.publish(topic, key, payload).await
    }

    /// Publish a batch of events.
    ///
    /// # Arguments
    /// * `events` - Vec of (topic, key, payload) tuples
    async fn publish_batch(
        &self,
        events: Vec<(String, Vec<u8>, Vec<u8>)>,
    ) -> Result<(), EventError> {
        // Default implementation publishes one at a time
        for (topic, key, payload) in events {
            self.publish(&topic, &key, &payload).await?;
        }
        Ok(())
    }

    /// Flush any buffered events.
    async fn flush(&self) -> Result<(), EventError> {
        Ok(())
    }
}

/// Topic-partition-offset for committing.
#[derive(Debug, Clone)]
pub struct TopicPartitionOffset {
    /// Topic name.
    pub topic: String,
    /// Partition number.
    pub partition: i32,
    /// Offset to commit.
    pub offset: i64,
}

/// Trait for event consumers.
///
/// Implementations consume events from a message transport.
#[async_trait]
pub trait EventConsumer: Send + Sync {
    /// Subscribe to one or more topics.
    ///
    /// # Arguments
    /// * `topics` - The topics to subscribe to
    async fn subscribe(&self, topics: &[&str]) -> Result<(), EventError>;

    /// Poll for new events.
    ///
    /// # Arguments
    /// * `timeout_ms` - Maximum time to wait for events in milliseconds
    ///
    /// # Returns
    /// A vector of received events (may be empty if timeout reached)
    async fn poll(&self, timeout_ms: u64) -> Result<Vec<RawEvent>, EventError>;

    /// Commit offsets for processed events.
    ///
    /// # Arguments
    /// * `offsets` - The offsets to commit (if empty, commits all consumed)
    async fn commit(&self, offsets: &[TopicPartitionOffset]) -> Result<(), EventError>;

    /// Commit all consumed offsets (convenience method).
    async fn commit_all(&self) -> Result<(), EventError> {
        self.commit(&[]).await
    }

    /// Unsubscribe from all topics.
    async fn unsubscribe(&self) -> Result<(), EventError>;

    /// Get the current consumer group offset lag.
    async fn lag(&self) -> Result<HashMap<String, i64>, EventError> {
        Ok(HashMap::new())
    }
}

/// Trait for event handlers.
///
/// Implement this to process specific event types.
#[async_trait]
pub trait EventHandler<E>: Send + Sync {
    /// Handle a single event.
    ///
    /// # Arguments
    /// * `event` - The deserialized event
    ///
    /// # Returns
    /// Ok(()) if the event was processed successfully
    async fn handle(&self, event: E) -> Result<(), EventError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_event_key_str() {
        let event = RawEvent {
            topic: "test".to_string(),
            partition: 0,
            offset: 0,
            key: Some(b"test-key".to_vec()),
            payload: vec![],
            headers: HashMap::new(),
            timestamp: None,
        };

        assert_eq!(event.key_str(), Some("test-key"));
    }

    #[test]
    fn test_raw_event_header() {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        let event = RawEvent {
            topic: "test".to_string(),
            partition: 0,
            offset: 0,
            key: None,
            payload: vec![],
            headers,
            timestamp: None,
        };

        assert_eq!(event.header("content-type"), Some("application/json"));
        assert_eq!(event.header("missing"), None);
    }
}
