//! Shard Executor Trait and Backend Implementations
//!
//! This crate defines the unified interface for executing model shards
//! across heterogeneous hardware backends.

use nexus_common::*;
use nexus_proto::*;
use async_trait::async_trait;
use candle_core::{Device, Tensor, DType, Result as CandleResult};
use std::sync::Arc;
use tracing::{info, debug};

/// Unified trait for shard execution backends
#[async_trait]
pub trait ShardExecutor: Send + Sync {
    /// Backend identifier
    fn backend_name(&self) -> &'static str;

    /// Load a shard from manifest
    async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()>;

    /// Forward pass through the shard
    async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch>;

    /// Get current KV cache
    fn get_kv_cache(&self) -> KVCacheHandle;

    /// Set KV cache (for pipeline continuity)
    fn set_kv_cache(&mut self, cache: KVCacheHandle);

    /// Get memory usage statistics
    fn memory_usage(&self) -> MemoryStats;

    /// Get device info
    fn device_info(&self) -> DeviceInfo;

    /// Warm up the executor (compile kernels, allocate memory)
    async fn warmup(&mut self) -> Result<()>;

    /// Shutdown and release resources
    async fn shutdown(&mut self) -> Result<()>;
}

/// Tensor batch for pipeline transfer
#[derive(Debug, Clone)]
pub struct TensorBatch {
    pub tensors: Vec<NamedTensor>,
    pub sequence_length: usize,
    pub batch_size: usize,
}

#[derive(Debug, Clone)]
pub struct NamedTensor {
    pub name: String,
    pub tensor: Tensor,
}

/// KV cache handle for inter-stage transfer
#[derive(Debug, Clone)]
pub struct KVCacheHandle {
    pub layers: Vec<LayerKVCache>,
    pub compressed: bool,
    pub compression_config: Option<CompressionConfig>,
}

#[derive(Debug, Clone)]
pub struct LayerKVCache {
    pub layer_idx: usize,
    pub keys: Tensor,   // [batch, seq_len, num_heads, head_dim]
    pub values: Tensor, // [batch, seq_len, num_heads, head_dim]
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub vram_used_bytes: u64,
    pub vram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub backend: String,
    pub compute_capability: Option<String>,
    pub memory_bandwidth_gbps: f64,
}

/// Factory for creating the best available executor
pub async fn create_best_executor(
    capabilities: &DeviceCapabilities,
    config: &crate::config::AgentConfig,
) -> Result<Arc<dyn ShardExecutor>> {
    // Priority order: CUDA > Metal > ROCm > CPU (llama.cpp) > WASM
    
    // Try CUDA first
    #[cfg(feature = "cuda")]
    {
        for device in &capabilities.compute_devices {
            if device.type_ == 1 { // CUDA
                info!("Creating CUDA executor for {}", device.name);
                return Ok(Arc::new(cuda_backend::CudaExecutor::new(device, config).await?));
            }
        }
    }

    // Try Metal (macOS)
    #[cfg(target_os = "macos")]
    {
        for device in &capabilities.compute_devices {
            if device.type_ == 3 { // METAL
                info!("Creating Metal executor for {}", device.name);
                return Ok(Arc::new(metal_backend::MetalExecutor::new(device, config).await?));
            }
        }
    }

    // Try ROCm
    #[cfg(feature = "rocm")]
    {
        for device in &capabilities.compute_devices {
            if device.type_ == 2 { // ROCM
                info!("Creating ROCm executor for {}", device.name);
                return Ok(Arc::new(rocm_backend::RocmExecutor::new(device, config).await?));
            }
        }
    }

    // Try llama.cpp (CPU with quantization)
    #[cfg(feature = "llama-cpp")]
    {
        info!("Creating llama.cpp executor (CPU)");
        return Ok(Arc::new(llama_cpp_backend::LlamaCppExecutor::new(config).await?));
    }

    // Fallback to CPU executor using candle
    info!("Creating CPU executor (candle)");
    Ok(Arc::new(cpu_backend::CpuExecutor::new(config).await?))
}

/// CUDA Backend
#[cfg(feature = "cuda")]
pub mod cuda_backend {
    use super::*;
    use candle_core::{Device, Tensor, DType};
    use cudarc::driver::{CudaDevice, CudaSlice, DevicePtr};
    use std::sync::Arc;

    pub struct CudaExecutor {
        device: Arc<CudaDevice>,
        candle_device: Device,
        shard_manifest: Option<ShardManifest>,
        model_weights: HashMap<String, Tensor>,
        kv_cache: Option<KVCacheHandle>,
        config: crate::config::AgentConfig,
    }

    impl CudaExecutor {
        pub async fn new(device: &ComputeDevice, config: &crate::config::AgentConfig) -> Result<Self> {
            let cuda_device = Arc::new(CudaDevice::new(0)?); // Use first GPU
            let candle_device = Device::new_cuda(0)?;

            Ok(Self {
                device: cuda_device,
                candle_device,
                shard_manifest: None,
                model_weights: HashMap::new(),
                kv_cache: None,
                config: config.clone(),
            })
        }
    }

    #[async_trait]
    impl ShardExecutor for CudaExecutor {
        fn backend_name(&self) -> &'static str { "cuda" }

        async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()> {
            info!("Loading shard {} on CUDA", manifest.shard_id);
            
            // Download and load weights from manifest
            for tensor_info in &manifest.tensors {
                // In production: download from IPFS/CDN, verify hash, load to GPU
                // For now, create placeholder
                let shape: Vec<usize> = tensor_info.shape.iter().map(|&x| x as usize).collect();
                let tensor = Tensor::zeros(&shape, DType::F16, &self.candle_device)?;
                self.model_weights.insert(tensor_info.name.clone(), tensor);
            }

            self.shard_manifest = Some(manifest.clone());
            Ok(())
        }

        async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch> {
            debug!("CUDA forward pass: {} tensors", input.tensors.len());
            
            // Execute the shard's layers
            // This would use candle-transformers or custom kernels
            let mut output_tensors = Vec::new();
            
            for named_tensor in input.tensors {
                // Placeholder: identity pass
                output_tensors.push(named_tensor);
            }

            Ok(TensorBatch {
                tensors: output_tensors,
                sequence_length: input.sequence_length,
                batch_size: input.batch_size,
            })
        }

        fn get_kv_cache(&self) -> KVCacheHandle {
            self.kv_cache.clone().unwrap_or_default()
        }

        fn set_kv_cache(&mut self, cache: KVCacheHandle) {
            self.kv_cache = Some(cache);
        }

        fn memory_usage(&self) -> MemoryStats {
            let (used, total) = self.device.memory_usage().unwrap_or((0, 0));
            MemoryStats {
                vram_used_bytes: used,
                vram_total_bytes: total,
                ram_used_bytes: 0,
                ram_total_bytes: 0,
            }
        }

        fn device_info(&self) -> DeviceInfo {
            DeviceInfo {
                name: "CUDA GPU".into(),
                backend: "cuda".into(),
                compute_capability: None,
                memory_bandwidth_gbps: 0.0,
            }
        }

        async fn warmup(&mut self) -> Result<()> {
            info!("Warming up CUDA executor");
            // Compile kernels, allocate workspace
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            info!("Shutting down CUDA executor");
            self.model_weights.clear();
            self.kv_cache = None;
            Ok(())
        }
    }
}

/// Metal Backend (Apple Silicon)
#[cfg(target_os = "macos")]
pub mod metal_backend {
    use super::*;
    use candle_core::{Device, Tensor, DType};
    use std::collections::HashMap;

    pub struct MetalExecutor {
        device: Device,
        shard_manifest: Option<ShardManifest>,
        model_weights: HashMap<String, Tensor>,
        kv_cache: Option<KVCacheHandle>,
        config: crate::config::AgentConfig,
    }

    impl MetalExecutor {
        pub async fn new(device: &ComputeDevice, config: &crate::config::AgentConfig) -> Result<Self> {
            let metal_device = Device::new_metal(0)?;

            Ok(Self {
                device: metal_device,
                shard_manifest: None,
                model_weights: HashMap::new(),
                kv_cache: None,
                config: config.clone(),
            })
        }
    }

    #[async_trait]
    impl ShardExecutor for MetalExecutor {
        fn backend_name(&self) -> &'static str { "metal" }

        async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()> {
            info!("Loading shard {} on Metal", manifest.shard_id);
            
            for tensor_info in &manifest.tensors {
                let shape: Vec<usize> = tensor_info.shape.iter().map(|&x| x as usize).collect();
                let tensor = Tensor::zeros(&shape, DType::F16, &self.device)?;
                self.model_weights.insert(tensor_info.name.clone(), tensor);
            }

            self.shard_manifest = Some(manifest.clone());
            Ok(())
        }

        async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch> {
            let mut output_tensors = Vec::new();
            for named_tensor in input.tensors {
                output_tensors.push(named_tensor);
            }
            Ok(TensorBatch {
                tensors: output_tensors,
                sequence_length: input.sequence_length,
                batch_size: input.batch_size,
            })
        }

        fn get_kv_cache(&self) -> KVCacheHandle {
            self.kv_cache.clone().unwrap_or_default()
        }

        fn set_kv_cache(&mut self, cache: KVCacheHandle) {
            self.kv_cache = Some(cache);
        }

        fn memory_usage(&self) -> MemoryStats {
            MemoryStats {
                vram_used_bytes: 0, // Unified memory
                vram_total_bytes: 0,
                ram_used_bytes: 0,
                ram_total_bytes: 0,
            }
        }

        fn device_info(&self) -> DeviceInfo {
            DeviceInfo {
                name: "Apple Metal".into(),
                backend: "metal".into(),
                compute_capability: None,
                memory_bandwidth_gbps: 400.0, // M3 Max ~400 GB/s
            }
        }

        async fn warmup(&mut self) -> Result<()> {
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            self.model_weights.clear();
            Ok(())
        }
    }
}

/// ROCm Backend (AMD GPU)
#[cfg(feature = "rocm")]
pub mod rocm_backend {
    use super::*;

    pub struct RocmExecutor {
        // ROCm uses candle with hip backend
        device: Device,
        shard_manifest: Option<ShardManifest>,
        model_weights: HashMap<String, Tensor>,
        kv_cache: Option<KVCacheHandle>,
        config: crate::config::AgentConfig,
    }

    impl RocmExecutor {
        pub async fn new(device: &ComputeDevice, config: &crate::config::AgentConfig) -> Result<Self> {
            let rocm_device = Device::new_rocm(0)?;

            Ok(Self {
                device: rocm_device,
                shard_manifest: None,
                model_weights: HashMap::new(),
                kv_cache: None,
                config: config.clone(),
            })
        }
    }

    #[async_trait]
    impl ShardExecutor for RocmExecutor {
        fn backend_name(&self) -> &'static str { "rocm" }

        async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()> {
            info!("Loading shard {} on ROCm", manifest.shard_id);
            Ok(())
        }

        async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch> {
            Ok(input)
        }

        fn get_kv_cache(&self) -> KVCacheHandle {
            self.kv_cache.clone().unwrap_or_default()
        }

        fn set_kv_cache(&mut self, cache: KVCacheHandle) {
            self.kv_cache = Some(cache);
        }

        fn memory_usage(&self) -> MemoryStats {
            MemoryStats::default()
        }

        fn device_info(&self) -> DeviceInfo {
            DeviceInfo {
                name: "AMD ROCm".into(),
                backend: "rocm".into(),
                compute_capability: None,
                memory_bandwidth_gbps: 0.0,
            }
        }

        async fn warmup(&mut self) -> Result<()> { Ok(()) }
        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

/// llama.cpp Backend (CPU with GGUF quantization)
#[cfg(feature = "llama-cpp")]
pub mod llama_cpp_backend {
    use super::*;
    use std::collections::HashMap;

    pub struct LlamaCppExecutor {
        model_path: String,
        context: Option<llama_cpp::LlamaContext>,
        kv_cache: Option<KVCacheHandle>,
        config: crate::config::AgentConfig,
    }

    impl LlamaCppExecutor {
        pub async fn new(config: &crate::config::AgentConfig) -> Result<Self> {
            Ok(Self {
                model_path: String::new(),
                context: None,
                kv_cache: None,
                config: config.clone(),
            })
        }
    }

    #[async_trait]
    impl ShardExecutor for LlamaCppExecutor {
        fn backend_name(&self) -> &'static str { "llama.cpp" }

        async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()> {
            info!("Loading shard {} on llama.cpp", manifest.shard_id);
            // Load GGUF model
            Ok(())
        }

        async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch> {
            Ok(input)
        }

        fn get_kv_cache(&self) -> KVCacheHandle {
            self.kv_cache.clone().unwrap_or_default()
        }

        fn set_kv_cache(&mut self, cache: KVCacheHandle) {
            self.kv_cache = Some(cache);
        }

        fn memory_usage(&self) -> MemoryStats {
            MemoryStats::default()
        }

        fn device_info(&self) -> DeviceInfo {
            DeviceInfo {
                name: "CPU (llama.cpp)".into(),
                backend: "llama.cpp".into(),
                compute_capability: None,
                memory_bandwidth_gbps: 50.0, // DDR5 ~50 GB/s
            }
        }

        async fn warmup(&mut self) -> Result<()> { Ok(()) }
        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

/// CPU Backend (candle native)
pub mod cpu_backend {
    use super::*;
    use candle_core::{Device, Tensor, DType};
    use std::collections::HashMap;

    pub struct CpuExecutor {
        device: Device,
        shard_manifest: Option<ShardManifest>,
        model_weights: HashMap<String, Tensor>,
        kv_cache: Option<KVCacheHandle>,
        config: crate::config::AgentConfig,
    }

    impl CpuExecutor {
        pub async fn new(config: &crate::config::AgentConfig) -> Result<Self> {
            let cpu_device = Device::Cpu;

            Ok(Self {
                device: cpu_device,
                shard_manifest: None,
                model_weights: HashMap::new(),
                kv_cache: None,
                config: config.clone(),
            })
        }
    }

    #[async_trait]
    impl ShardExecutor for CpuExecutor {
        fn backend_name(&self) -> &'static str { "cpu" }

        async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()> {
            info!("Loading shard {} on CPU", manifest.shard_id);
            
            for tensor_info in &manifest.tensors {
                let shape: Vec<usize> = tensor_info.shape.iter().map(|&x| x as usize).collect();
                let dtype = match tensor_info.dtype as i32 {
                    1 => DType::F16,
                    2 => DType::BF16,
                    3 => DType::F32,
                    _ => DType::F32,
                };
                let tensor = Tensor::zeros(&shape, dtype, &self.device)?;
                self.model_weights.insert(tensor_info.name.clone(), tensor);
            }

            self.shard_manifest = Some(manifest.clone());
            Ok(())
        }

        async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch> {
            let mut output_tensors = Vec::new();
            for named_tensor in input.tensors {
                output_tensors.push(named_tensor);
            }
            Ok(TensorBatch {
                tensors: output_tensors,
                sequence_length: input.sequence_length,
                batch_size: input.batch_size,
            })
        }

        fn get_kv_cache(&self) -> KVCacheHandle {
            self.kv_cache.clone().unwrap_or_default()
        }

        fn set_kv_cache(&mut self, cache: KVCacheHandle) {
            self.kv_cache = Some(cache);
        }

        fn memory_usage(&self) -> MemoryStats {
            MemoryStats::default()
        }

        fn device_info(&self) -> DeviceInfo {
            DeviceInfo {
                name: "CPU".into(),
                backend: "cpu".into(),
                compute_capability: None,
                memory_bandwidth_gbps: 50.0,
            }
        }

        async fn warmup(&mut self) -> Result<()> { Ok(()) }
        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

impl Default for KVCacheHandle {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            compressed: false,
            compression_config: None,
        }
    }
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            vram_used_bytes: 0,
            vram_total_bytes: 0,
            ram_used_bytes: 0,
            ram_total_bytes: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_batch_creation() {
        let batch = TensorBatch {
            tensors: vec![],
            sequence_length: 512,
            batch_size: 1,
        };
        assert_eq!(batch.sequence_length, 512);
    }
}