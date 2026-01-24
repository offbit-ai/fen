//! Local in-memory event bus for standalone mode and testing.
//!
//! This provides a simple in-process event transport that doesn't require
//! Kafka or any external dependencies.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::traits::{EventConsumer, EventError, EventProducer, RawEvent, TopicPartitionOffset};

/// Configuration for the local event bus.
#[derive(Debug, Clone)]
pub struct LocalEventBusConfig {
    /// Channel capacity per topic.
    pub channel_capacity: usize,
}

impl Default for LocalEventBusConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1000,
        }
    }
}

/// In-memory event bus for local/testing use.
///
/// Uses tokio broadcast channels for pub/sub within a process.
pub struct LocalEventBus {
    /// Topic -> broadcast sender
    topics: Arc<RwLock<HashMap<String, broadcast::Sender<RawEvent>>>>,
    /// Channel capacity
    capacity: usize,
    /// Offset counter per topic
    offsets: Arc<RwLock<HashMap<String, AtomicI64>>>,
}

impl LocalEventBus {
    /// Create a new local event bus with default configuration.
    pub fn new() -> Self {
        Self::with_config(LocalEventBusConfig::default())
    }

    /// Create a new local event bus with custom configuration.
    pub fn with_config(config: LocalEventBusConfig) -> Self {
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
            capacity: config.channel_capacity,
            offsets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a broadcast sender for a topic.
    async fn get_or_create_sender(&self, topic: &str) -> broadcast::Sender<RawEvent> {
        let topics = self.topics.read().await;
        if let Some(sender) = topics.get(topic) {
            return sender.clone();
        }
        drop(topics);

        let mut topics = self.topics.write().await;
        // Double-check after acquiring write lock
        if let Some(sender) = topics.get(topic) {
            return sender.clone();
        }

        let (sender, _) = broadcast::channel(self.capacity);
        topics.insert(topic.to_string(), sender.clone());

        // Initialize offset counter
        let mut offsets = self.offsets.write().await;
        offsets.insert(topic.to_string(), AtomicI64::new(0));

        sender
    }

    /// Get next offset for a topic.
    async fn next_offset(&self, topic: &str) -> i64 {
        let offsets = self.offsets.read().await;
        if let Some(offset) = offsets.get(topic) {
            offset.fetch_add(1, Ordering::SeqCst)
        } else {
            0
        }
    }
}

impl Default for LocalEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventProducer for LocalEventBus {
    async fn publish(&self, topic: &str, key: &[u8], payload: &[u8]) -> Result<(), EventError> {
        let sender = self.get_or_create_sender(topic).await;
        let offset = self.next_offset(topic).await;

        let event = RawEvent {
            topic: topic.to_string(),
            partition: 0, // Single partition for local bus
            offset,
            key: Some(key.to_vec()),
            payload: payload.to_vec(),
            headers: HashMap::new(),
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
        };

        // Ignore send error if no receivers (that's fine for pub/sub)
        let _ = sender.send(event);
        Ok(())
    }

    async fn publish_with_headers(
        &self,
        topic: &str,
        key: &[u8],
        payload: &[u8],
        headers: HashMap<String, String>,
    ) -> Result<(), EventError> {
        let sender = self.get_or_create_sender(topic).await;
        let offset = self.next_offset(topic).await;

        let event = RawEvent {
            topic: topic.to_string(),
            partition: 0,
            offset,
            key: Some(key.to_vec()),
            payload: payload.to_vec(),
            headers,
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
        };

        let _ = sender.send(event);
        Ok(())
    }
}

/// Local event consumer for the in-memory bus.
pub struct LocalEventConsumer {
    /// Reference to the event bus
    bus: Arc<LocalEventBus>,
    /// Active receivers per topic
    receivers: RwLock<HashMap<String, broadcast::Receiver<RawEvent>>>,
    /// Subscribed topics
    subscribed_topics: RwLock<Vec<String>>,
}

impl LocalEventConsumer {
    /// Create a new consumer for the given event bus.
    pub fn new(bus: Arc<LocalEventBus>) -> Self {
        Self {
            bus,
            receivers: RwLock::new(HashMap::new()),
            subscribed_topics: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl EventConsumer for LocalEventConsumer {
    async fn subscribe(&self, topics: &[&str]) -> Result<(), EventError> {
        let mut receivers = self.receivers.write().await;
        let mut subscribed = self.subscribed_topics.write().await;

        for topic in topics {
            let sender = self.bus.get_or_create_sender(topic).await;
            receivers.insert(topic.to_string(), sender.subscribe());
            subscribed.push(topic.to_string());
        }

        Ok(())
    }

    async fn poll(&self, timeout_ms: u64) -> Result<Vec<RawEvent>, EventError> {
        let mut events = Vec::new();
        let timeout = tokio::time::Duration::from_millis(timeout_ms);

        let mut receivers = self.receivers.write().await;

        for receiver in receivers.values_mut() {
            match tokio::time::timeout(timeout, receiver.recv()).await {
                Ok(Ok(event)) => events.push(event),
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    tracing::warn!("Consumer lagged by {} messages", n);
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    // Channel closed, continue
                }
                Err(_) => {
                    // Timeout, continue
                }
            }
        }

        Ok(events)
    }

    async fn commit(&self, _offsets: &[TopicPartitionOffset]) -> Result<(), EventError> {
        // No-op for local bus (no persistent offsets)
        Ok(())
    }

    async fn unsubscribe(&self) -> Result<(), EventError> {
        let mut receivers = self.receivers.write().await;
        let mut subscribed = self.subscribed_topics.write().await;

        receivers.clear();
        subscribed.clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_event_bus_publish_subscribe() {
        let bus = Arc::new(LocalEventBus::new());
        let consumer = LocalEventConsumer::new(bus.clone());

        // Subscribe to topic
        consumer.subscribe(&["test-topic"]).await.unwrap();

        // Publish event
        bus.publish("test-topic", b"key1", b"payload1")
            .await
            .unwrap();

        // Poll for event
        let events = consumer.poll(100).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].topic, "test-topic");
        assert_eq!(events[0].key, Some(b"key1".to_vec()));
        assert_eq!(events[0].payload, b"payload1".to_vec());
    }

    #[tokio::test]
    async fn test_local_event_bus_multiple_topics() {
        let bus = Arc::new(LocalEventBus::new());
        let consumer = LocalEventConsumer::new(bus.clone());

        consumer
            .subscribe(&["topic-a", "topic-b"])
            .await
            .unwrap();

        bus.publish("topic-a", b"key-a", b"payload-a")
            .await
            .unwrap();
        bus.publish("topic-b", b"key-b", b"payload-b")
            .await
            .unwrap();

        // Poll should get events from both topics
        let mut events = Vec::new();
        for _ in 0..2 {
            events.extend(consumer.poll(100).await.unwrap());
        }

        assert!(events.iter().any(|e| e.topic == "topic-a"));
        assert!(events.iter().any(|e| e.topic == "topic-b"));
    }
}
