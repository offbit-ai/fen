//! Kafka consumer implementation.

use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::TopicPartitionList;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::config::KafkaConfig;
use crate::traits::{EventConsumer, EventError, RawEvent, TopicPartitionOffset};

/// Kafka consumer for receiving events.
pub struct KafkaConsumer {
    consumer: Arc<StreamConsumer>,
    config: KafkaConfig,
    subscribed_topics: RwLock<Vec<String>>,
}

impl KafkaConsumer {
    /// Create a new Kafka consumer.
    pub fn new(config: KafkaConfig) -> Result<Self, EventError> {
        let mut client_config = ClientConfig::new();

        client_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("group.id", &config.consumer_group)
            .set("auto.offset.reset", &config.auto_offset_reset)
            .set("enable.auto.commit", config.enable_auto_commit.to_string())
            .set(
                "auto.commit.interval.ms",
                config.auto_commit_interval_ms.to_string(),
            )
            .set("security.protocol", &config.security_protocol);

        if let Some(ref mechanism) = config.sasl_mechanism {
            client_config.set("sasl.mechanism", mechanism);
        }
        if let Some(ref username) = config.sasl_username {
            client_config.set("sasl.username", username);
        }
        if let Some(ref password) = config.sasl_password {
            client_config.set("sasl.password", password);
        }
        if let Some(ref ca_location) = config.ssl_ca_location {
            client_config.set("ssl.ca.location", ca_location);
        }

        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| EventError::Configuration(e.to_string()))?;

        Ok(Self {
            consumer: Arc::new(consumer),
            config,
            subscribed_topics: RwLock::new(Vec::new()),
        })
    }

    /// Get the configuration.
    pub fn config(&self) -> &KafkaConfig {
        &self.config
    }
}

#[async_trait]
impl EventConsumer for KafkaConsumer {
    async fn subscribe(&self, topics: &[&str]) -> Result<(), EventError> {
        let topic_refs: Vec<&str> = topics.to_vec();

        self.consumer
            .subscribe(&topic_refs)
            .map_err(|e| EventError::Subscribe(e.to_string()))?;

        let mut subscribed = self.subscribed_topics.write().await;
        subscribed.clear();
        subscribed.extend(topics.iter().map(|t| t.to_string()));

        Ok(())
    }

    async fn poll(&self, timeout_ms: u64) -> Result<Vec<RawEvent>, EventError> {
        let timeout = Duration::from_millis(timeout_ms);
        let mut events = Vec::new();

        // Poll for messages
        match tokio::time::timeout(timeout, async {
            match self.consumer.recv().await {
                Ok(msg) => {
                    let headers = msg
                        .headers()
                        .map(|h| {
                            let mut map = HashMap::new();
                            for header in h.iter() {
                                if let Some(value) = header.value {
                                    if let Ok(v) = std::str::from_utf8(value) {
                                        map.insert(header.key.to_string(), v.to_string());
                                    }
                                }
                            }
                            map
                        })
                        .unwrap_or_default();

                    let event = RawEvent {
                        topic: msg.topic().to_string(),
                        partition: msg.partition(),
                        offset: msg.offset(),
                        key: msg.key().map(|k| k.to_vec()),
                        payload: msg.payload().map(|p| p.to_vec()).unwrap_or_default(),
                        headers,
                        timestamp: msg.timestamp().to_millis(),
                    };
                    events.push(event);
                }
                Err(e) => {
                    return Err(EventError::Consume(e.to_string()));
                }
            }
            Ok(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                // Timeout, return empty
            }
        }

        Ok(events)
    }

    async fn commit(&self, offsets: &[TopicPartitionOffset]) -> Result<(), EventError> {
        if offsets.is_empty() {
            self.consumer
                .commit_consumer_state(rdkafka::consumer::CommitMode::Sync)
                .map_err(|e| EventError::Commit(e.to_string()))?;
        } else {
            let mut tpl = TopicPartitionList::new();
            for offset in offsets {
                tpl.add_partition_offset(
                    &offset.topic,
                    offset.partition,
                    rdkafka::Offset::Offset(offset.offset + 1),
                )
                .map_err(|e| EventError::Commit(e.to_string()))?;
            }

            self.consumer
                .commit(&tpl, rdkafka::consumer::CommitMode::Sync)
                .map_err(|e| EventError::Commit(e.to_string()))?;
        }

        Ok(())
    }

    async fn unsubscribe(&self) -> Result<(), EventError> {
        self.consumer.unsubscribe();
        let mut subscribed = self.subscribed_topics.write().await;
        subscribed.clear();
        Ok(())
    }

    async fn lag(&self) -> Result<HashMap<String, i64>, EventError> {
        let mut lag_map = HashMap::new();

        let subscribed = self.subscribed_topics.read().await;
        for topic in subscribed.iter() {
            // Get committed offsets
            if let Ok(committed) = self.consumer.committed(Duration::from_secs(5)) {
                if let Ok(watermarks) =
                    self.consumer
                        .fetch_watermarks(topic, 0, Duration::from_secs(5))
                {
                    let high = watermarks.1;
                    let committed_offset = committed
                        .find_partition(topic, 0)
                        .map(|p| p.offset().to_raw().unwrap_or(0))
                        .unwrap_or(0);
                    lag_map.insert(topic.clone(), high - committed_offset);
                }
            }
        }

        Ok(lag_map)
    }
}
