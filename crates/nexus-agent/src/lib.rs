//! Nexus Agent - Worker daemon for distributed LLM inference
//!
//! This crate implements the client-side agent that runs on worker nodes,
//! managing device registration, task execution, and result submission.

use nexus_common::*;
use nexus_executor::ShardExecutor;
use nexus_p2p::P2PNode;
use nexus_proto::*;
use nexus_tee::TEEManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn, error, debug, instrument};

pub mod config;
pub mod capabilities;
pub mod task_runner;
pub mod heartbeat;
pub mod attestation;

use config::AgentConfig;
use capabilities::discover_capabilities;
use task_runner::TaskRunner;
use heartbeat::HeartbeatManager;
use attestation::AttestationManager;

/// Main agent state
pub struct NexusAgent {
    config: AgentConfig,
    identity: HardwareIdentity,
    capabilities: DeviceCapabilities,
    p2p_node: Arc<P2PNode>,
    executor: Arc<dyn ShardExecutor>,
    tee_manager: Option<Arc<TEEManager>>,
    task_runner: TaskRunner,
    heartbeat_manager: HeartbeatManager,
    attestation_manager: AttestationManager,
    running: Arc<RwLock<bool>>,
}

impl NexusAgent {
    /// Create a new agent instance
    #[instrument(skip(config))]
    pub async fn new(config: AgentConfig) -> Result<Self> {
        info!("Initializing Nexus Agent v{}", env!("CARGO_PKG_VERSION"));

        // Derive hardware identity (TPM-bound if available)
        let identity = capabilities::derive_hardware_identity(&config).await?;
        info!("Hardware identity: {}", identity.peer_id);

        // Discover device capabilities
        let capabilities = discover_capabilities(&config).await?;
        info!("Discovered capabilities: {} compute devices, {} GB system RAM",
              capabilities.compute_devices.len(),
              capabilities.system_ram_bytes / (1024*1024*1024));

        // Initialize P2P node
        let p2p_node = Arc::new(P2PNode::new(&config, &identity).await?);
        info!("P2P node started: {}", p2p_node.peer_id());

        // Initialize TEE manager if available
        let tee_manager = if config.tee_enabled && capabilities.tee.available {
            let mgr = TEEManager::new(&capabilities.tee).await?;
            info!("TEE manager initialized: {:?}", capabilities.tee.types);
            Some(Arc::new(mgr))
        } else {
            warn!("TEE not available or disabled");
            None
        };

        // Create shard executor based on best available backend
        let executor = nexus_executor::create_best_executor(&capabilities, &config).await?;
        info!("Executor initialized: {}", executor.backend_name());

        // Initialize sub-components
        let task_runner = TaskRunner::new(
            executor.clone(),
            p2p_node.clone(),
            tee_manager.clone(),
            config.clone(),
        );

        let heartbeat_manager = HeartbeatManager::new(
            p2p_node.clone(),
            identity.clone(),
            capabilities.clone(),
            config.heartbeat_interval_secs,
        );

        let attestation_manager = AttestationManager::new(
            tee_manager.clone(),
            p2p_node.clone(),
            config.attestation_interval_secs,
        );

        Ok(Self {
            config,
            identity,
            capabilities,
            p2p_node,
            executor,
            tee_manager,
            task_runner,
            heartbeat_manager,
            attestation_manager,
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the agent main loop
    #[instrument(skip(self))]
    pub async fn run(&self) -> Result<()> {
        *self.running.write().await = true;
        info!("Nexus Agent starting...");

        // Register with orchestrator
        self.register_with_orchestrator().await?;

        // Start background tasks
        let heartbeat_handle = self.heartbeat_manager.start(self.running.clone());
        let attestation_handle = self.attestation_manager.start(self.running.clone());
        let task_poll_handle = self.task_runner.start_polling(self.running.clone());

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("Shutdown signal received");

        *self.running.write().await = false;

        // Graceful shutdown
        heartbeat_handle.await??;
        attestation_handle.await??;
        task_poll_handle.await??;

        self.p2p_node.shutdown().await?;
        info!("Nexus Agent stopped gracefully");
        Ok(())
    }

    /// Register worker with the orchestrator
    async fn register_with_orchestrator(&self) -> Result<()> {
        let request = RegisterWorkerRequest {
            identity: self.identity.clone(),
            capabilities: self.capabilities.clone(),
            stake: self.config.stake_info.clone(),
            metadata_json: serde_json::to_string(&self.config.metadata)?,
        };

        let response = self.p2p_node.call_rpc("register_worker", &request).await?;

        if !response.accepted {
            return Err(anyhow::anyhow!("Registration rejected: {}", response.error));
        }

        info!("Registered with orchestrator: worker_id={}", response.worker_id);
        Ok(())
    }

    /// Get current agent status
    pub async fn status(&self) -> AgentStatus {
        AgentStatus {
            peer_id: self.identity.peer_id.clone(),
            running: *self.running.read().await,
            capabilities: self.capabilities.clone(),
            current_task: self.task_runner.current_task().await,
            p2p_peers: self.p2p_node.connected_peers().await,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    pub peer_id: String,
    pub running: bool,
    pub capabilities: DeviceCapabilities,
    pub current_task: Option<String>,
    pub p2p_peers: Vec<String>,
}

/// Agent configuration
pub mod config {
    use super::*;
    use std::path::PathBuf;

    #[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
    pub struct AgentConfig {
        pub orchestrator_url: String,
        pub model_cache_dir: PathBuf,
        pub tee_enabled: bool,
        pub heartbeat_interval_secs: u64,
        pub attestation_interval_secs: u64,
        pub max_concurrent_tasks: usize,
        pub stake_info: StakeInfo,
        pub metadata: HashMap<String, String>,
        pub log_level: String,
        pub metrics_port: u16,
    }

    impl Default for AgentConfig {
        fn default() -> Self {
            Self {
                orchestrator_url: "wss://orchestrator.nexusllm.network".into(),
                model_cache_dir: PathBuf::from("./model_cache"),
                tee_enabled: true,
                heartbeat_interval_secs: 10,
                attestation_interval_secs: 300,
                max_concurrent_tasks: 1,
                stake_info: StakeInfo::default(),
                metadata: HashMap::new(),
                log_level: "info".into(),
                metrics_port: 9090,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_config_default() {
        let config = config::AgentConfig::default();
        assert_eq!(config.heartbeat_interval_secs, 10);
        assert!(config.tee_enabled);
    }
}