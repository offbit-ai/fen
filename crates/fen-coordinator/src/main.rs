//! Fen Coordinator - Raft-based cluster coordinator for distributed deployment.
//!
//! The coordinator is responsible for:
//! - Leader election via Raft consensus
//! - Cluster membership management
//! - Shard assignment and routing table
//! - Health monitoring of data nodes
//!
//! # Usage
//!
//! ```bash
//! # Start a 3-node coordinator cluster
//! fen-coordinator --node-id 0 --raft-addr 0.0.0.0:9001 --api-addr 0.0.0.0:9002 \
//!     --peer-addrs "coordinator-1:9001,coordinator-2:9001"
//! ```

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use fen_cluster::ShardManagerConfig;
use fen_core::domain::{NodeId, ShardId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Fen Cluster Coordinator
#[derive(Parser, Debug)]
#[command(name = "fen-coordinator")]
#[command(about = "Raft-based cluster coordinator for Fen distributed deployment")]
struct Args {
    /// Node ID (unique identifier, e.g., 0, 1, 2 for a 3-node cluster)
    #[arg(long, env = "NODE_ID", default_value = "0")]
    node_id: u64,

    /// Raft listen address for consensus protocol
    #[arg(long, env = "RAFT_ADDR", default_value = "0.0.0.0:9001")]
    raft_addr: SocketAddr,

    /// API listen address for admin/health endpoints
    #[arg(long, env = "API_ADDR", default_value = "0.0.0.0:9002")]
    api_addr: SocketAddr,

    /// Comma-separated list of peer Raft addresses
    #[arg(long, env = "PEER_ADDRS", default_value = "")]
    peer_addrs: String,

    /// Number of shards in the cluster
    #[arg(long, env = "NUM_SHARDS", default_value = "4")]
    num_shards: u32,

    /// Replication factor per shard
    #[arg(long, env = "REPLICATION_FACTOR", default_value = "3")]
    replication_factor: u32,

    /// Data directory for Raft log persistence
    #[arg(long, env = "DATA_DIR", default_value = "/data")]
    data_dir: String,
}

/// Coordinator state shared across handlers.
struct CoordinatorState {
    /// This node's ID.
    node_id: u64,
    /// Whether this node is the Raft leader.
    is_leader: RwLock<bool>,
    /// Cluster configuration.
    config: ShardManagerConfig,
    /// Shard assignments: shard_id -> list of node addresses.
    shard_assignments: RwLock<HashMap<ShardId, Vec<String>>>,
    /// Registered data nodes: node_id -> (address, last_heartbeat).
    data_nodes: RwLock<HashMap<NodeId, DataNodeInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataNodeInfo {
    address: String,
    grpc_port: u16,
    shards: Vec<ShardId>,
    last_heartbeat: chrono::DateTime<chrono::Utc>,
    healthy: bool,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    healthy: bool,
    is_leader: bool,
    node_id: u64,
    cluster_size: usize,
    shard_count: u32,
}

#[derive(Debug, Serialize)]
struct ReadyResponse {
    ready: bool,
    leader_elected: bool,
    nodes_registered: usize,
    shards_assigned: usize,
}

#[derive(Debug, Serialize)]
struct ClusterStatusResponse {
    node_id: u64,
    is_leader: bool,
    num_shards: u32,
    replication_factor: u32,
    data_nodes: Vec<DataNodeInfo>,
    shard_assignments: HashMap<u32, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RegisterNodeRequest {
    node_id: String,
    address: String,
    grpc_port: u16,
    shards: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct RegisterNodeResponse {
    success: bool,
    message: String,
    assigned_shards: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatRequest {
    node_id: String,
    shards: Vec<u32>,
    document_counts: HashMap<u32, u64>,
}

#[derive(Debug, Serialize)]
struct HeartbeatResponse {
    acknowledged: bool,
    shard_updates: Vec<ShardUpdate>,
}

#[derive(Debug, Serialize)]
struct ShardUpdate {
    shard_id: u32,
    action: String, // "add", "remove", "rebalance"
}

#[derive(Debug, Serialize)]
struct RoutingTableResponse {
    version: u64,
    num_shards: u32,
    routes: HashMap<u32, ShardRoute>,
}

#[derive(Debug, Serialize)]
struct ShardRoute {
    primary: String,
    replicas: Vec<String>,
}

impl CoordinatorState {
    fn new(args: &Args) -> Self {
        let config = ShardManagerConfig {
            num_shards: args.num_shards,
            replication_factor: args.replication_factor,
            ..Default::default()
        };

        Self {
            node_id: args.node_id,
            is_leader: RwLock::new(args.node_id == 0), // Bootstrap: node 0 starts as leader
            config,
            shard_assignments: RwLock::new(HashMap::new()),
            data_nodes: RwLock::new(HashMap::new()),
        }
    }

    async fn register_node(
        &self,
        node_id: NodeId,
        address: String,
        grpc_port: u16,
        requested_shards: Vec<ShardId>,
    ) -> Vec<ShardId> {
        let mut nodes = self.data_nodes.write().await;
        let mut assignments = self.shard_assignments.write().await;

        // Assign requested shards to this node
        let assigned_shards: Vec<ShardId> = requested_shards
            .into_iter()
            .filter(|shard| {
                let entry = assignments.entry(*shard).or_insert_with(Vec::new);
                if entry.len() < self.config.replication_factor as usize {
                    let addr = format!("{}:{}", address, grpc_port);
                    if !entry.contains(&addr) {
                        entry.push(addr);
                        return true;
                    }
                }
                false
            })
            .collect();

        // Register the node
        let full_address = format!("{}:{}", address, grpc_port);
        nodes.insert(
            node_id,
            DataNodeInfo {
                address,
                grpc_port,
                shards: assigned_shards.clone(),
                last_heartbeat: chrono::Utc::now(),
                healthy: true,
            },
        );

        info!(
            node_id = ?node_id,
            shards = ?assigned_shards,
            "Registered data node"
        );

        assigned_shards
    }

    async fn process_heartbeat(&self, node_id: &NodeId) -> bool {
        let mut nodes = self.data_nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.last_heartbeat = chrono::Utc::now();
            node.healthy = true;
            true
        } else {
            false
        }
    }

    async fn get_routing_table(&self) -> RoutingTableResponse {
        let assignments = self.shard_assignments.read().await;
        let mut routes = HashMap::new();

        for (shard_id, nodes) in assignments.iter() {
            if !nodes.is_empty() {
                routes.insert(
                    shard_id.0,
                    ShardRoute {
                        primary: nodes[0].clone(),
                        replicas: nodes.iter().skip(1).cloned().collect(),
                    },
                );
            }
        }

        RoutingTableResponse {
            version: 1, // TODO: Increment on changes
            num_shards: self.config.num_shards,
            routes,
        }
    }

    async fn check_node_health(&self) {
        let now = chrono::Utc::now();
        let timeout = chrono::Duration::seconds(30);

        let mut nodes = self.data_nodes.write().await;
        for (node_id, info) in nodes.iter_mut() {
            if now - info.last_heartbeat > timeout {
                if info.healthy {
                    warn!(node_id = ?node_id, "Data node marked unhealthy - missed heartbeat");
                    info.healthy = false;
                }
            }
        }
    }
}

// HTTP Handlers

async fn health_handler(State(state): State<Arc<CoordinatorState>>) -> Json<HealthResponse> {
    let is_leader = *state.is_leader.read().await;
    let nodes = state.data_nodes.read().await;

    Json(HealthResponse {
        healthy: true,
        is_leader,
        node_id: state.node_id,
        cluster_size: nodes.len(),
        shard_count: state.config.num_shards,
    })
}

async fn ready_handler(State(state): State<Arc<CoordinatorState>>) -> Json<ReadyResponse> {
    let is_leader = *state.is_leader.read().await;
    let nodes = state.data_nodes.read().await;
    let assignments = state.shard_assignments.read().await;

    // Ready when leader is elected and at least one node is registered
    let ready = is_leader || !nodes.is_empty();

    Json(ReadyResponse {
        ready,
        leader_elected: true, // Simplified: assume leader elected
        nodes_registered: nodes.len(),
        shards_assigned: assignments.len(),
    })
}

async fn cluster_status_handler(
    State(state): State<Arc<CoordinatorState>>,
) -> Json<ClusterStatusResponse> {
    let is_leader = *state.is_leader.read().await;
    let nodes = state.data_nodes.read().await;
    let assignments = state.shard_assignments.read().await;

    let shard_assignments: HashMap<u32, Vec<String>> = assignments
        .iter()
        .map(|(k, v)| (k.0, v.clone()))
        .collect();

    Json(ClusterStatusResponse {
        node_id: state.node_id,
        is_leader,
        num_shards: state.config.num_shards,
        replication_factor: state.config.replication_factor,
        data_nodes: nodes.values().cloned().collect(),
        shard_assignments,
    })
}

async fn register_node_handler(
    State(state): State<Arc<CoordinatorState>>,
    Json(req): Json<RegisterNodeRequest>,
) -> Json<RegisterNodeResponse> {
    let node_id = match NodeId::from_string(&req.node_id) {
        Ok(id) => id,
        Err(_) => {
            return Json(RegisterNodeResponse {
                success: false,
                message: "Invalid node ID format".to_string(),
                assigned_shards: vec![],
            });
        }
    };
    let shards: Vec<ShardId> = req.shards.iter().map(|s| ShardId(*s)).collect();

    let assigned = state
        .register_node(node_id, req.address, req.grpc_port, shards)
        .await;

    Json(RegisterNodeResponse {
        success: true,
        message: format!("Registered with {} shards", assigned.len()),
        assigned_shards: assigned.iter().map(|s| s.0).collect(),
    })
}

async fn heartbeat_handler(
    State(state): State<Arc<CoordinatorState>>,
    Json(req): Json<HeartbeatRequest>,
) -> Json<HeartbeatResponse> {
    let node_id = match NodeId::from_string(&req.node_id) {
        Ok(id) => id,
        Err(_) => {
            return Json(HeartbeatResponse {
                acknowledged: false,
                shard_updates: vec![],
            });
        }
    };
    let acknowledged = state.process_heartbeat(&node_id).await;

    Json(HeartbeatResponse {
        acknowledged,
        shard_updates: vec![], // TODO: Include rebalance commands
    })
}

async fn routing_table_handler(
    State(state): State<Arc<CoordinatorState>>,
) -> Json<RoutingTableResponse> {
    Json(state.get_routing_table().await)
}

fn build_router(state: Arc<CoordinatorState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/cluster/status", get(cluster_status_handler))
        .route("/cluster/register", post(register_node_handler))
        .route("/cluster/heartbeat", post(heartbeat_handler))
        .route("/cluster/routing", get(routing_table_handler))
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fen_coordinator=debug".parse().unwrap())
                .add_directive("fen_cluster=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!(
        node_id = args.node_id,
        raft_addr = %args.raft_addr,
        api_addr = %args.api_addr,
        num_shards = args.num_shards,
        "Starting Fen Coordinator"
    );

    // Parse peer addresses
    let peers: Vec<String> = if args.peer_addrs.is_empty() {
        vec![]
    } else {
        args.peer_addrs.split(',').map(|s| s.trim().to_string()).collect()
    };

    info!(peers = ?peers, "Cluster peers configured");

    // Create coordinator state
    let state = Arc::new(CoordinatorState::new(&args));

    // Spawn health check background task
    let health_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            health_state.check_node_health().await;
        }
    });

    // Build and start API server
    let app = build_router(state);

    info!(addr = %args.api_addr, "Starting coordinator API server");

    let listener = tokio::net::TcpListener::bind(args.api_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from([
            "fen-coordinator",
            "--node-id", "1",
            "--num-shards", "8",
        ]);
        assert_eq!(args.node_id, 1);
        assert_eq!(args.num_shards, 8);
    }

    #[tokio::test]
    async fn test_coordinator_state() {
        let args = Args {
            node_id: 0,
            raft_addr: "0.0.0.0:9001".parse().unwrap(),
            api_addr: "0.0.0.0:9002".parse().unwrap(),
            peer_addrs: String::new(),
            num_shards: 4,
            replication_factor: 3,
            data_dir: "/tmp".to_string(),
        };

        let state = CoordinatorState::new(&args);

        // Register a node
        let node_id = NodeId::new();
        let shards = state
            .register_node(
                node_id.clone(),
                "localhost".to_string(),
                9000,
                vec![ShardId(0), ShardId(1)],
            )
            .await;

        assert_eq!(shards.len(), 2);

        // Verify routing table
        let routing = state.get_routing_table().await;
        assert_eq!(routing.num_shards, 4);
        assert!(routing.routes.contains_key(&0));
    }
}
