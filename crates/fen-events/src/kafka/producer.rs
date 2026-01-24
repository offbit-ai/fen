//! Kafka producer implementation.

use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::collections::HashMap;
use std::time::Duration;

use super::config::KafkaConfig;
use crate::traits::{EventError, EventProducer};

/// Kafka producer for publishing events.
pub struct KafkaProducer {
    producer: FutureProducer,
    config: KafkaConfig,
}

impl KafkaProducer {
    /// Create a new Kafka producer.
    pub fn new(config: KafkaConfig) -> Result<Self, EventError> {
        let mut client_config = ClientConfig::new();

        client_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("message.timeout.ms", config.message_timeout_ms.to_string())
            .set("acks", &config.acks)
            .set("enable.idempotence", config.enable_idempotence.to_string())
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

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| EventError::Configuration(e.to_string()))?;

        Ok(Self { producer, config })
    }

    /// Get the configuration.
    pub fn config(&self) -> &KafkaConfig {
        &self.config
    }
}

#[async_trait]
impl EventProducer for KafkaProducer {
    async fn publish(&self, topic: &str, key: &[u8], payload: &[u8]) -> Result<(), EventError> {
        let record = FutureRecord::to(topic).key(key).payload(payload);

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| EventError::Publish(e.to_string()))?;

        Ok(())
    }

    async fn publish_with_headers(
        &self,
        topic: &str,
        key: &[u8],
        payload: &[u8],
        headers: HashMap<String, String>,
    ) -> Result<(), EventError> {
        use rdkafka::message::OwnedHeaders;

        let mut owned_headers = OwnedHeaders::new();
        for (k, v) in headers {
            owned_headers = owned_headers.insert(rdkafka::message::Header {
                key: &k,
                value: Some(v.as_bytes()),
            });
        }

        let record = FutureRecord::to(topic)
            .key(key)
            .payload(payload)
            .headers(owned_headers);

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| EventError::Publish(e.to_string()))?;

        Ok(())
    }

    async fn publish_batch(
        &self,
        events: Vec<(String, Vec<u8>, Vec<u8>)>,
    ) -> Result<(), EventError> {
        let futures: Vec<_> = events
            .into_iter()
            .map(|(topic, key, payload)| {
                let record = FutureRecord::to(&topic)
                    .key(key.as_slice())
                    .payload(payload.as_slice());
                self.producer.send(record, Duration::from_secs(5))
            })
            .collect();

        for result in futures::future::join_all(futures).await {
            result.map_err(|(e, _)| EventError::Publish(e.to_string()))?;
        }

        Ok(())
    }

    async fn flush(&self) -> Result<(), EventError> {
        self.producer
            .flush(Duration::from_secs(30))
            .map_err(|e| EventError::Publish(format!("Flush failed: {}", e)))
    }
}
