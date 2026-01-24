//! GoRules Zen Engine integration with thread isolation
//!
//! The zen-engine uses QuickJS (via rquickjs) which is not Send/Sync.
//! We work around this by running the engine in a dedicated thread and
//! communicating via channels.

use std::path::Path;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};
use zen_engine::handler::custom_node_adapter::NoopCustomNode;
use zen_engine::loader::NoopLoader;
use zen_engine::model::DecisionContent;
use zen_engine::Decision;

use crate::error::RuleError;

/// Type alias for the Zen decision with default loader and no custom nodes
type ZenDecision = Decision<NoopLoader, NoopCustomNode>;

/// Request sent to the rule engine worker thread
enum RuleRequest {
    /// Evaluate a decision with given input
    Evaluate {
        input: JsonValue,
        response_tx: oneshot::Sender<Result<JsonValue, RuleError>>,
    },
    /// Shutdown the worker
    Shutdown,
}

/// Handle to the rule engine running in a dedicated thread
/// This type is Send + Sync and can be safely shared across async tasks
pub struct ZenEngineHandle {
    request_tx: mpsc::UnboundedSender<RuleRequest>,
    _worker_handle: Arc<JoinHandle<()>>,
}

impl ZenEngineHandle {
    /// Create a new zen engine handle by loading rules from a file
    pub async fn new(rules_path: &Path) -> Result<Self, RuleError> {
        // Read the rules file content
        let rules_content = tokio::fs::read_to_string(rules_path)
            .await
            .map_err(|e| RuleError::Loading(format!("Failed to read rules file: {}", e)))?;

        // Parse the decision content
        let decision: DecisionContent = serde_json::from_str(&rules_content)
            .map_err(|e| RuleError::Loading(format!("Failed to parse rules: {}", e)))?;

        Self::from_decision(decision)
    }

    /// Create a zen engine handle from decision content
    pub fn from_decision(decision: DecisionContent) -> Result<Self, RuleError> {
        let (request_tx, request_rx) = mpsc::unbounded_channel();

        // Spawn the worker thread
        let worker_handle = thread::spawn(move || {
            run_worker(decision, request_rx);
        });

        tracing::info!("Zen engine worker thread started");

        Ok(Self {
            request_tx,
            _worker_handle: Arc::new(worker_handle),
        })
    }

    /// Evaluate the decision with the given input
    pub async fn evaluate(&self, input: JsonValue) -> Result<JsonValue, RuleError> {
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(RuleRequest::Evaluate { input, response_tx })
            .map_err(|_| RuleError::Evaluation("Worker thread not available".to_string()))?;

        response_rx
            .await
            .map_err(|_| RuleError::Evaluation("Worker thread did not respond".to_string()))?
    }

    /// Shutdown the engine gracefully
    pub fn shutdown(&self) {
        let _ = self.request_tx.send(RuleRequest::Shutdown);
    }
}

impl Drop for ZenEngineHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Worker function that runs in a dedicated thread
fn run_worker(decision_content: DecisionContent, mut request_rx: mpsc::UnboundedReceiver<RuleRequest>) {
    // Create the decision from content - this is !Send so must stay on this thread
    let decision: ZenDecision = decision_content.into();

    // Create a runtime for this thread to receive messages
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create worker runtime");

    rt.block_on(async {
        tracing::debug!("Zen engine worker ready");

        while let Some(request) = request_rx.recv().await {
            match request {
                RuleRequest::Evaluate { input, response_tx } => {
                    let result = evaluate_decision(&decision, input).await;
                    let _ = response_tx.send(result);
                }
                RuleRequest::Shutdown => {
                    tracing::debug!("Zen engine worker shutting down");
                    break;
                }
            }
        }
    });
}

/// Evaluate the decision asynchronously
async fn evaluate_decision(
    decision: &ZenDecision,
    input: JsonValue,
) -> Result<JsonValue, RuleError> {
    let result = decision
        .evaluate(&input)
        .await
        .map_err(|e| RuleError::Evaluation(format!("Evaluation failed: {}", e)))?;

    Ok(result.result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_decision() -> DecisionContent {
        // Create a simple test decision that checks if amount > 0
        serde_json::from_value(json!({
            "nodes": [
                {
                    "id": "input",
                    "name": "Request",
                    "type": "inputNode"
                },
                {
                    "id": "output",
                    "name": "Response",
                    "type": "outputNode"
                },
                {
                    "id": "rule1",
                    "name": "Amount Check",
                    "type": "decisionTableNode",
                    "content": {
                        "hitPolicy": "first",
                        "inputs": [
                            {
                                "id": "amount",
                                "name": "Amount",
                                "field": "amount"
                            }
                        ],
                        "outputs": [
                            {
                                "id": "valid",
                                "name": "Valid",
                                "field": "valid"
                            }
                        ],
                        "rules": [
                            {
                                "amount": "> 0",
                                "valid": "true"
                            },
                            {
                                "amount": "",
                                "valid": "false"
                            }
                        ]
                    }
                }
            ],
            "edges": [
                { "id": "e1", "sourceId": "input", "targetId": "rule1" },
                { "id": "e2", "sourceId": "rule1", "targetId": "output" }
            ]
        }))
        .expect("Failed to parse test decision")
    }

    #[tokio::test]
    async fn test_zen_engine_handle() {
        let decision = create_test_decision();
        let handle = ZenEngineHandle::from_decision(decision).unwrap();

        let result = handle.evaluate(json!({ "amount": 100 })).await.unwrap();
        assert_eq!(result.get("valid"), Some(&json!(true)));

        let result = handle.evaluate(json!({ "amount": -5 })).await.unwrap();
        assert_eq!(result.get("valid"), Some(&json!(false)));
    }
}
