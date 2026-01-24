//! Notification delivery provider implementations.
//!
//! This module contains concrete implementations of the
//! `NotificationDeliveryProvider` trait for various delivery channels.

pub mod websocket;

#[cfg(feature = "email")]
pub mod email;

#[cfg(feature = "webhook")]
pub mod webhook;

pub use websocket::WebSocketProvider;

#[cfg(feature = "email")]
pub use email::EmailProvider;

#[cfg(feature = "webhook")]
pub use webhook::WebhookProvider;
