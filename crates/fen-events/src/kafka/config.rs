//! Kafka client configuration.

use serde::{Deserialize, Serialize};

/// Kafka client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    /// Comma-separated list of Kafka brokers.
    pub bootstrap_servers: String,

    /// Consumer group ID.
    pub consumer_group: String,

    /// Auto offset reset policy: "earliest" or "latest".
    #[serde(default = "default_auto_offset_reset")]
    pub auto_offset_reset: String,

    /// Enable auto commit.
    #[serde(default)]
    pub enable_auto_commit: bool,

    /// Auto commit interval in milliseconds.
    #[serde(default = "default_auto_commit_interval")]
    pub auto_commit_interval_ms: u32,

    /// Message timeout in milliseconds.
    #[serde(default = "default_message_timeout")]
    pub message_timeout_ms: u32,

    /// Request timeout in milliseconds.
    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u32,

    /// Enable idempotent producer.
    #[serde(default = "default_enable_idempotence")]
    pub enable_idempotence: bool,

    /// Producer acks: "0", "1", or "all".
    #[serde(default = "default_acks")]
    pub acks: String,

    /// Security protocol: "PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL".
    #[serde(default = "default_security_protocol")]
    pub security_protocol: String,

    /// SASL mechanism: "PLAIN", "SCRAM-SHA-256", "SCRAM-SHA-512".
    pub sasl_mechanism: Option<String>,

    /// SASL username.
    pub sasl_username: Option<String>,

    /// SASL password.
    pub sasl_password: Option<String>,

    /// Path to SSL CA certificate.
    pub ssl_ca_location: Option<String>,
}

fn default_auto_offset_reset() -> String {
    "earliest".to_string()
}

fn default_auto_commit_interval() -> u32 {
    5000
}

fn default_message_timeout() -> u32 {
    30000
}

fn default_request_timeout() -> u32 {
    30000
}

fn default_enable_idempotence() -> bool {
    true
}

fn default_acks() -> String {
    "all".to_string()
}

fn default_security_protocol() -> String {
    "PLAINTEXT".to_string()
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            consumer_group: "fen-default".to_string(),
            auto_offset_reset: default_auto_offset_reset(),
            enable_auto_commit: false,
            auto_commit_interval_ms: default_auto_commit_interval(),
            message_timeout_ms: default_message_timeout(),
            request_timeout_ms: default_request_timeout(),
            enable_idempotence: default_enable_idempotence(),
            acks: default_acks(),
            security_protocol: default_security_protocol(),
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            ssl_ca_location: None,
        }
    }
}

impl KafkaConfig {
    /// Create a new Kafka configuration.
    pub fn new(bootstrap_servers: &str, consumer_group: &str) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.to_string(),
            consumer_group: consumer_group.to_string(),
            ..Default::default()
        }
    }

    /// Set SASL credentials.
    pub fn with_sasl(mut self, mechanism: &str, username: &str, password: &str) -> Self {
        self.security_protocol = "SASL_SSL".to_string();
        self.sasl_mechanism = Some(mechanism.to_string());
        self.sasl_username = Some(username.to_string());
        self.sasl_password = Some(password.to_string());
        self
    }

    /// Set SSL CA certificate location.
    pub fn with_ssl_ca(mut self, ca_location: &str) -> Self {
        self.ssl_ca_location = Some(ca_location.to_string());
        self
    }
}
