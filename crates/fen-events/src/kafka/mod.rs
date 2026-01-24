//! Kafka producer and consumer implementations.
//!
//! This module is only available when the `kafka` feature is enabled.

mod config;
mod consumer;
mod producer;

pub use config::KafkaConfig;
pub use consumer::KafkaConsumer;
pub use producer::KafkaProducer;
