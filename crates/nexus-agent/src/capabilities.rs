//! Device capability discovery and hardware identity derivation

use nexus_common::*;
use nexus_proto::*;
use std::collections::HashMap;
use sysinfo::{System, CpuExt, DiskExt, NetworkExt};
use tracing::{info, warn, debug};

/// Derive hardware identity bound to TPM/HSM if available
pub async fn derive_hardware_identity(config: &crate::config::AgentConfig) -> Result<HardwareIdentity> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Get network interfaces for MAC-based fingerprint
    let networks = sys.networks();
    let primary_mac = networks.values()
        .find(|n| !n.mac_address().is_empty())
        .map(|n| n.mac_address().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get CPU info for fingerprint
    let cpu_brand = sys.cpus().first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Create hardware fingerprint
    let fingerprint_input = format!("{}{}{}", primary_mac, cpu_brand, uuid::Uuid::new_v4());
    let hardware_fingerprint = blake3::hash(fingerprint_input.as_bytes()).as_bytes().to_vec();

    // Try to get TPM endorsement key (platform-specific)
    let tpm_ek_pub = get_tpm_ek_pub().await.unwrap_or_default();

    // Determine TEE type
    let (tee_type, tee_measurement) = detect_tee().await;

    let identity = HardwareIdentity {
        peer_id: generate_peer_id(&hardware_fingerprint),
        tpm_ek_pub,
        hardware_fingerprint,
        tee_type,
        tee_measurement,
        timestamp: chrono::Utc::now().timestamp() as u64,
        signature: Vec::new(), // Will be signed by TPM/HSM
    };

    // Sign with TPM if available
    let signed_identity = sign_with_tpm(identity).await?;

    Ok(signed_identity)
}

/// Discover all device capabilities
pub async fn discover_capabilities(config: &crate::config::AgentConfig) -> Result<DeviceCapabilities> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU info
    let cpu_cores = sys.cpus().len() as u32;
    let cpu_arch = std::env::consts::ARCH.to_string();
    let cpu_features = detect_cpu_features();

    // Memory
    let system_ram_bytes = sys.total_memory();
    let swap_bytes = sys.total_swap();

    // Disks
    let (nvme_bytes, hdd_bytes) = scan_storage(&sys);

    // Network
    let network = discover_network_info().await;

    // Compute devices (GPUs, NPUs)
    let compute_devices = discover_compute_devices().await;

    // TEE capabilities
    let tee = detect_tee_capabilities().await;

    // Power profile
    let power = detect_power_profile().await;

    Ok(DeviceCapabilities {
        compute_devices,
        cpu_cores,
        cpu_arch,
        cpu_features,
        system_ram_bytes,
        swap_bytes,
        memory_regions: vec![], // Filled by compute device discovery
        nvme_bytes,
        hdd_bytes,
        cache_path: config.model_cache_dir.to_string_lossy().to_string(),
        network,
        tee,
        power,
    })
}

fn detect_cpu_features() -> Vec<String> {
    let mut features = Vec::new();
    
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") { features.push("avx2".into()); }
        if is_x86_feature_detected!("avx512f") { features.push("avx512f".into()); }
        if is_x86_feature_detected!("avx512bw") { features.push("avx512bw".into()); }
        if is_x86_feature_detected!("avx512vl") { features.push("avx512vl".into()); }
        if is_x86_feature_detected!("amx") { features.push("amx".into()); }
        if is_x86_feature_detected!("fma") { features.push("fma".into()); }
        if is_x86_feature_detected!("bmi1") { features.push("bmi1".into()); }
        if is_x86_feature_detected!("bmi2") { features.push("bmi2".into()); }
    }
    
    #[cfg(target_arch = "aarch64")]
    {
        // NEON is always available on aarch64
        features.push("neon".into());
        // SVE detection would require runtime check
    }

    features
}

fn scan_storage(sys: &System) -> (u64, u64) {
    let mut nvme = 0u64;
    let mut hdd = 0u64;

    for disk in sys.disks() {
        let name = disk.name().to_string_lossy().to_lowercase();
        let space = disk.total_space();
        
        if name.contains("nvme") || name.contains("ssd") {
            nvme += space;
        } else {
            hdd += space;
        }
    }

    (nvme, hdd)
}

async fn discover_network_info() -> NetworkInfo {
    // In production, this would use STUN to detect NAT type and public IP
    // For now, return defaults
    NetworkInfo {
        nat_type: 0, // UNKNOWN
        public_ip: "auto".into(),
        public_port: 0,
        relay_peers: vec![],
        bandwidth_mbps_up: 100.0,
        bandwidth_mbps_down: 1000.0,
        latency_ms_to_coordinators: 50,
    }
}

async fn discover_compute_devices() -> Vec<ComputeDevice> {
    let mut devices = Vec::new();

    // NVIDIA CUDA
    #[cfg(feature = "cuda")]
    {
        if let Ok(cuda_devices) = discover_cuda_devices().await {
            devices.extend(cuda_devices);
        }
    }

    // AMD ROCm
    #[cfg(feature = "rocm")]
    {
        if let Ok(rocm_devices) = discover_rocm_devices().await {
            devices.extend(rocm_devices);
        }
    }

    // Apple Metal
    #[cfg(target_os = "macos")]
    {
        if let Ok(metal_devices) = discover_metal_devices().await {
            devices.extend(metal_devices);
        }
    }

    // CPU fallback
    if devices.is_empty() {
        devices.push(cpu_compute_device());
    }

    devices
}

#[cfg(feature = "cuda")]
async fn discover_cuda_devices() -> Result<Vec<ComputeDevice>> {
    // Would use cuda-sys or cudarc to enumerate devices
    // Placeholder implementation
    Ok(vec![])
}

#[cfg(feature = "rocm")]
async fn discover_rocm_devices() -> Result<Vec<ComputeDevice>> {
    Ok(vec![])
}

#[cfg(target_os = "macos")]
async fn discover_metal_devices() -> Result<Vec<ComputeDevice>> {
    // Use metal crate to enumerate GPUs
    Ok(vec![])
}

fn cpu_compute_device() -> ComputeDevice {
    ComputeDevice {
        type_: 5, // CPU
        name: "CPU".into(),
        compute_units: num_cpus::get() as u32,
        vram_bytes: 0,
        shared_mem_bytes: 0, // Will be filled from system RAM
        supported_precisions: vec!["fp32".into(), "int8".into(), "int4".into()],
        tflops_fp16: 0.0,
        tflops_int8: 0.0,
        driver_version: "native".into(),
        pcie_gen: 0,
        pcie_lanes: 0,
    }
}

async fn detect_tee() -> (String, Vec<u8>) {
    // Check for available TEEs
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/sgx_enclave").exists() || std::path::Path::new("/dev/sgx_provision").exists() {
            return ("sgx".into(), get_sgx_measurement().await);
        }
        if std::path::Path::new("/dev/sev").exists() {
            return ("sev".into(), get_sev_measurement().await);
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        return ("secure_enclave".into(), get_secure_enclave_measurement().await);
    }

    ("none".into(), vec![])
}

async fn get_sgx_measurement() -> Vec<u8> {
    // Would use sgx-urts to get MRENCLAVE
    vec![]
}

async fn get_sev_measurement() -> Vec<u8> {
    // Would use sev-rs to get measurement
    vec![]
}

async fn get_secure_enclave_measurement() -> Vec<u8> {
    // Would use security-framework
    vec![]
}

async fn detect_tee_capabilities() -> TEECapabilities {
    let mut types = Vec::new();
    
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/sgx_enclave").exists() { types.push("sgx".into()); }
        if std::path::Path::new("/dev/sev").exists() { types.push("sev-snp".into()); }
    }
    
    #[cfg(target_os = "macos")]
    {
        types.push("secure_enclave".into());
    }
    
    #[cfg(target_os = "android")]
    {
        types.push("trustzone".into());
    }

    TEECapabilities {
        available: !types.is_empty(),
        types,
        max_enclave_size_bytes: 128 * 1024 * 1024, // 128MB typical
        remote_attestation: true,
        sealing: true,
        key_derivation: true,
    }
}

async fn detect_power_profile() -> PowerProfile {
    // Would read from sysfs / Windows WMI / macOS IOKit
    PowerProfile {
        max_power_watts: 500.0,
        idle_power_watts: 50.0,
        thermal_throttling: true,
        max_temp_celsius: 85,
    }
}

fn generate_peer_id(fingerprint: &[u8]) -> String {
    use libp2p::PeerId;
    use multihash::{Multihash, Code};
    
    let hash = blake3::hash(fingerprint);
    let multihash = Multihash::wrap(Code::Blake3_256, hash.as_bytes()).unwrap();
    PeerId::from_multihash(multihash).unwrap().to_string()
}

async fn get_tpm_ek_pub() -> Option<Vec<u8>> {
    // Platform-specific TPM access
    // Linux: tss2, Windows: TPM Base Services, macOS: Secure Enclave
    None
}

async fn sign_with_tpm(mut identity: HardwareIdentity) -> Result<HardwareIdentity> {
    // Sign identity with TPM/HSM
    // For now, use software signing as fallback
    let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
    let verifying_key = signing_key.verifying_key();
    
    let msg = bincode::serialize(&identity)?;
    let signature = signing_key.sign(&msg).to_bytes().to_vec();
    
    identity.signature = signature;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_features_detection() {
        let features = detect_cpu_features();
        #[cfg(target_arch = "x86_64")]
        assert!(features.contains(&"avx2".to_string()) || features.contains(&"neon".to_string()));
    }

    #[test]
    fn test_peer_id_generation() {
        let fp = b"test_fingerprint";
        let peer_id = generate_peer_id(fp);
        assert!(peer_id.starts_with("12D3KooW")); // libp2p PeerId prefix
    }
}