//! Model and Worker registry for NexusLLM

pub mod model_registry;
pub mod worker_registry;

use nexus_common::*;
use nexus_proto::*;

pub struct ModelRegistryClient {
    // TODO: client state
}

impl ModelRegistryClient {
    pub fn new(url: &str) -> Self {
        Self {}
    }

    pub async fn fetch_shard_manifest(&self, model_id: &str, shard_id: &str) -> Result<ShardManifest> {
        todo!("Fetch shard manifest from registry")
    }
}

pub struct WorkerRegistryClient {
    // TODO: client state
}

impl WorkerRegistryClient {
    pub fn new(url: &str) -> Self {
        Self {}
    }

    pub async fn register(&self, identity: &HardwareIdentity, capabilities: &DeviceCapabilities) -> Result<String> {
        todo!("Register worker")
    }

    pub async fn poll_task(&self, identity: &HardwareIdentity) -> Result<TaskAssignment> {
        todo!("Poll for assigned task")
    }

    pub async fn submit_result(&self, task_id: &str, result: ExecutionResult, proof: Vec<u8>) -> Result<()> {
        todo!("Submit task result")
    }
}