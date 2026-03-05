//! Event bus integration tests.
//!
//! Verifies publish → subscribe → consume round-trips for domain events,
//! including JSON serialization of typed payloads.

use std::collections::HashMap;
use std::sync::Arc;

use fen_events::traits::{EventConsumer, EventProducer};
use fen_events::{LocalEventBus, LocalEventConsumer};

#[tokio::test]
async fn domain_event_json_round_trip() {
    let bus = Arc::new(LocalEventBus::new());
    let consumer = LocalEventConsumer::new(bus.clone());

    consumer.subscribe(&["document.processed"]).await.unwrap();

    // Simulate a domain event payload
    let payload = serde_json::json!({
        "document_id": "d1234",
        "tenant_id": "t5678",
        "event_type": "document.processed",
        "invoice_number": "INV-001",
        "total_amount": 1500.00,
    });

    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    bus.publish("document.processed", b"d1234", &payload_bytes)
        .await
        .unwrap();

    let events = consumer.poll(200).await.unwrap();
    assert_eq!(events.len(), 1);

    let received: serde_json::Value = serde_json::from_slice(&events[0].payload).unwrap();
    assert_eq!(received["document_id"], "d1234");
    assert_eq!(received["invoice_number"], "INV-001");
    assert_eq!(received["total_amount"], 1500.00);
}

#[tokio::test]
async fn publish_with_headers_preserves_metadata() {
    let bus = Arc::new(LocalEventBus::new());
    let consumer = LocalEventConsumer::new(bus.clone());

    consumer.subscribe(&["anomaly.detected"]).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("source".to_string(), "rule-engine".to_string());
    headers.insert("severity".to_string(), "high".to_string());

    bus.publish_with_headers("anomaly.detected", b"a999", b"anomaly payload", headers)
        .await
        .unwrap();

    let events = consumer.poll(200).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].headers.get("source").unwrap(), "rule-engine");
    assert_eq!(events[0].headers.get("severity").unwrap(), "high");
    assert_eq!(events[0].payload, b"anomaly payload");
}

#[tokio::test]
async fn multiple_subscribers_receive_independently() {
    let bus = Arc::new(LocalEventBus::new());
    let consumer_a = LocalEventConsumer::new(bus.clone());
    let consumer_b = LocalEventConsumer::new(bus.clone());

    consumer_a.subscribe(&["shared.topic"]).await.unwrap();
    consumer_b.subscribe(&["shared.topic"]).await.unwrap();

    bus.publish("shared.topic", b"key", b"msg1").await.unwrap();

    let events_a = consumer_a.poll(200).await.unwrap();
    let events_b = consumer_b.poll(200).await.unwrap();

    assert_eq!(events_a.len(), 1);
    assert_eq!(events_b.len(), 1);
    assert_eq!(events_a[0].payload, events_b[0].payload);
}

#[tokio::test]
async fn offsets_increment_monotonically() {
    let bus = Arc::new(LocalEventBus::new());
    let consumer = LocalEventConsumer::new(bus.clone());

    consumer.subscribe(&["offset.test"]).await.unwrap();

    for i in 0..5 {
        bus.publish("offset.test", format!("k{i}").as_bytes(), b"data")
            .await
            .unwrap();
    }

    // Collect all events
    let mut all_events = Vec::new();
    for _ in 0..5 {
        all_events.extend(consumer.poll(100).await.unwrap());
    }

    assert_eq!(all_events.len(), 5);
    let offsets: Vec<i64> = all_events.iter().map(|e| e.offset).collect();
    for i in 1..offsets.len() {
        assert!(
            offsets[i] > offsets[i - 1],
            "Offsets must be monotonically increasing: {:?}",
            offsets
        );
    }
}

#[tokio::test]
async fn unsubscribe_stops_receiving() {
    let bus = Arc::new(LocalEventBus::new());
    let consumer = LocalEventConsumer::new(bus.clone());

    consumer.subscribe(&["unsub.topic"]).await.unwrap();

    bus.publish("unsub.topic", b"k1", b"before").await.unwrap();

    let events = consumer.poll(100).await.unwrap();
    assert_eq!(events.len(), 1);

    consumer.unsubscribe().await.unwrap();

    bus.publish("unsub.topic", b"k2", b"after").await.unwrap();

    let events = consumer.poll(100).await.unwrap();
    assert_eq!(events.len(), 0, "Should not receive after unsubscribe");
}
