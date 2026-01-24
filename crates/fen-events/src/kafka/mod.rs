//! Kafka producer and consumer implementations.
//!
//! This module is only available when the `kafka` feature is enabled.

mod producer;
mod consumer;
mod config;

pub use producer::KafkaProducer;
pub use consumer::KafkaConsumer;
pub use config::KafkaConfig;
