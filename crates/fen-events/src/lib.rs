//! Event schemas and transport for distributed Fen.
//!
//! This crate provides:
//! - Protobuf-generated event types for all Fen domain events
//! - Kafka topic definitions and configuration
//! - Producer and consumer traits with Kafka and local implementations
//!
//! # Features
//!
//! - `kafka` - Enable Kafka producer/consumer implementations (requires librdkafka)
//!
//! # Example
//!
//! ```ignore
//! use fen_events::{LocalEventBus, EventProducer, topics};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let bus = Arc::new(LocalEventBus::new());
//!
//!     // Publish an event
//!     bus.publish(
//!         topics::DOCUMENT_INGESTION,
//!         b"tenant-123:invoice",
//!         b"{\"document_id\": \"doc-1\"}",
//!     ).await.unwrap();
//! }
//! ```

pub mod local;
pub mod topics;
pub mod traits;

#[cfg(feature = "kafka")]
pub mod kafka;

// Re-export generated protobuf types
pub mod events {
    include!(concat!(env!("OUT_DIR"), "/fen.events.v1.rs"));
}

// Re-export commonly used types
pub use local::{LocalEventBus, LocalEventBusConfig, LocalEventConsumer};
pub use topics::*;
pub use traits::{
    EventConsumer, EventError, EventHandler, EventProducer, RawEvent, TopicPartitionOffset,
};

#[cfg(feature = "kafka")]
pub use kafka::{KafkaConfig, KafkaConsumer, KafkaProducer};
