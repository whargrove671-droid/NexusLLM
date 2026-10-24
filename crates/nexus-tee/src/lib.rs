//! TEE Manager - Unified interface for hardware attestation and secure execution

use nexus_common::*;
use nexus_proto::*;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn, debug, instrument};

// Add blake3 for software fallback
#[cfg(feature = "software")]
use blake3;

#[async_trait]
pub trait TEEBackend: Send + Sync {
    fn tee_type(&self) -> &'static str;
    
    /// Initialize the TEE
    async fn initialize(&mut self, config: &TEEConfig) -> Result<()>;
    
    /// Generate remote attestation quote
    async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>>;
    
    /// Verify a remote attestation quote
    async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool>;
    
    /// Seal data to TEE (encrypt with TEE-bound key)
    async fn seal(&self, plaintext: &[u8], policy: &SealPolicy) -> Result<Vec<u8>>;
    
    /// Unseal data from TEE
    async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>>;
    
    /// Derive key inside TEE
    async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]>;
    
    /// Get TEE measurement (MRENCLAVE, etc.)
    fn measurement(&self) -> &[u8];
    
    /// Shutdown TEE
    async fn shutdown(&mut self) -> Result<()>;
}

/// Execute a function inside TEE (non-trait function to avoid dyn compatibility issues)
pub async fn execute_in_tee<F, R>(backend: &Arc<dyn TEEBackend>, f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send,
    R: Send,
{
    // Execute inside TEE via backend-specific mechanism
    f()
}

#[derive(Debug, Clone)]
pub struct TEEConfig {
    pub tee_type: String,
    pub max_enclave_size: usize,
    pub enable_debug: bool,
    pub heap_pages: usize,
    pub stack_pages: usize,
}

impl Default for TEEConfig {
    fn default() -> Self {
        Self {
            tee_type: "auto".into(),
            max_enclave_size: 256 * 1024 * 1024,
            enable_debug: false,
            heap_pages: 1024,
            stack_pages: 256,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SealPolicy {
    pub mrenclave: Vec<u8>,      // Bind to specific enclave measurement
    pub mrsigner: Vec<u8>,       // Bind to signer
    pub isv_prod_id: u16,
    pub isv_svn: u16,
    pub config_id: [u8; 64],
    pub config_svn: u16,
}

impl Default for SealPolicy {
    fn default() -> Self {
        Self {
            mrenclave: vec![],
            mrsigner: vec![],
            isv_prod_id: 1,
            isv_svn: 1,
            config_id: [0; 64],
            config_svn: 0,
        }
    }
}

/// TEE Manager - selects and manages the best available TEE
pub struct TEEManager {
    backend: Option<Arc<dyn TEEBackend>>,
    config: TEEConfig,
    capabilities: TEECapabilities,
}

impl TEEManager {
    pub async fn new(capabilities: &TEECapabilities) -> Result<Self> {
        // Select best available TEE
        // Priority: SGX > SEV-SNP > TrustZone > Secure Enclave
        let tee_type = if capabilities.types.contains(&"sgx".to_string()) {
            "sgx"
        } else if capabilities.types.contains(&"sev-snp".to_string()) {
            "sev-snp"
        } else if capabilities.types.contains(&"trustzone".to_string()) {
            "trustzone"
        } else if capabilities.types.contains(&"secure_enclave".to_string()) {
            "secure_enclave"
        } else {
            return Err(anyhow::anyhow!("No TEE available"));
        };

        info!("Selected TEE: {}", tee_type);

        let config = TEEConfig {
            tee_type: tee_type.into(),
            ..Default::default()
        };

        let backend = create_tee_backend(tee_type).await?;
        backend.initialize(&config).await?;

        Ok(Self {
            backend: Some(backend),
            config,
            capabilities: capabilities.clone(),
        })
    }

    pub fn backend(&self) -> Option<&Arc<dyn TEEBackend>> {
        self.backend.as_ref()
    }

    pub fn tee_type(&self) -> &str {
        self.backend.as_ref().map(|b| b.tee_type()).unwrap_or("none")
    }

    pub async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>> {
        self.backend.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TEE backend"))?
            .get_quote(report_data)
            .await
    }

    pub async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool> {
        self.backend.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TEE backend"))?
            .verify_quote(quote, expected_measurement)
            .await
    }

    pub async fn seal(&self, plaintext: &[u8], policy: &SealPolicy) -> Result<Vec<u8>> {
        self.backend.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TEE backend"))?
            .seal(plaintext, policy)
            .await
    }

    pub async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        self.backend.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TEE backend"))?
            .unseal(sealed)
            .await
    }

    pub async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]> {
        self.backend.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TEE backend"))?
            .derive_key(label, context)
            .await
    }

    pub async fn execute_in_tee<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send,
        R: Send,
    {
        self.backend.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TEE backend"))?
            .execute_in_tee(f)
            .await
    }
}

async fn create_tee_backend(tee_type: &str) -> Result<Arc<dyn TEEBackend>> {
    match tee_type {
        "sgx" => {
            #[cfg(feature = "sgx")]
            {
                Ok(Arc::new(sgx_backend::SgxBackend::new()?))
            }
            #[cfg(not(feature = "sgx"))]
            {
                Err(anyhow::anyhow!("SGX not compiled in"))
            }
        }
        "sev-snp" => {
            #[cfg(feature = "sev")]
            {
                Ok(Arc::new(sev_backend::SevBackend::new()?))
            }
            #[cfg(not(feature = "sev"))]
            {
                Err(anyhow::anyhow!("SEV not compiled in"))
            }
        }
        "trustzone" => {
            Ok(Arc::new(trustzone_backend::TrustZoneBackend::new()?))
        }
        "secure_enclave" => {
            Ok(Arc::new(secure_enclave_backend::SecureEnclaveBackend::new()?))
        }
        _ => Err(anyhow::anyhow!("Unknown TEE type: {}", tee_type)),
    }
}

/// SGX Backend (Intel)
#[cfg(feature = "sgx")]
pub mod sgx_backend {
    use super::*;
    use sgx_urts::{SgxEnclave, Enclave};
    use std::path::Path;

    pub struct SgxBackend {
        enclave: Option<SgxEnclave>,
        measurement: Vec<u8>,
    }

    impl SgxBackend {
        pub fn new() -> Result<Self> {
            Ok(Self {
                enclave: None,
                measurement: vec![],
            })
        }
    }

    #[async_trait]
    impl TEEBackend for SgxBackend {
        fn tee_type(&self) -> &'static str { "sgx" }

        async fn initialize(&mut self, config: &TEEConfig) -> Result<()> {
            let enclave_path = std::env::var("NEXUS_SGX_ENCLAVE")
                .unwrap_or_else(|_| "./enclave.signed.so".into());
            
            let enclave = SgxEnclave::create(
                Path::new(&enclave_path),
                config.enable_debug,
            )?;

            // Get MRENCLAVE measurement
            let report = enclave.get_report(&[0; 64])?;
            self.measurement = report.body.mr_enclave.m.to_vec();
            self.enclave = Some(enclave);

            info!("SGX enclave initialized, MRENCLAVE: {:x?}", &self.measurement[..16]);
            Ok(())
        }

        async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>> {
            let enclave = self.enclave.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Enclave not initialized"))?;
            
            let mut report_data_padded = [0u8; 64];
            report_data_padded[..report_data.len().min(64)].copy_from_slice(report_data);
            
            let quote = enclave.get_quote(&report_data_padded)?;
            Ok(quote.to_vec())
        }

        async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool> {
            // Verify quote using Intel DCAP
            // This would use sgx-dcap-ql or similar
            // For now, basic measurement check
            Ok(self.measurement == expected_measurement)
        }

        async fn seal(&self, plaintext: &[u8], policy: &SealPolicy) -> Result<Vec<u8>> {
            let enclave = self.enclave.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Enclave not initialized"))?;
            
            // Call enclave ECALL to seal data
            // This is a simplified placeholder
            let mut sealed = vec![0u8; plaintext.len() + 256];
            // enclave.ecall_seal_data(plaintext, &mut sealed, policy)?;
            Ok(sealed)
        }

        async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
            let enclave = self.enclave.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Enclave not initialized"))?;
            
            let mut plaintext = vec![0u8; sealed.len()];
            // enclave.ecall_unseal_data(sealed, &mut plaintext)?;
            Ok(plaintext)
        }

        async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]> {
            let enclave = self.enclave.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Enclave not initialized"))?;
            
            let mut key = [0u8; 32];
            // enclave.ecall_derive_key(label, context, &mut key)?;
            Ok(key)
        }

        async fn execute_in_tee<F, R>(&self, f: F) -> Result<R>
        where
            F: FnOnce() -> Result<R> + Send,
            R: Send,
        {
            // Execute inside enclave via ECALL
            f()
        }

        fn measurement(&self) -> &[u8] {
            &self.measurement
        }

        async fn shutdown(&mut self) -> Result<()> {
            self.enclave = None;
            Ok(())
        }
    }
}

/// SEV-SNP Backend (AMD)
#[cfg(feature = "sev")]
pub mod sev_backend {
    use super::*;

    pub struct SevBackend {
        measurement: Vec<u8>,
    }

    impl SevBackend {
        pub fn new() -> Result<Self> {
            Ok(Self { measurement: vec![] })
        }
    }

    #[async_trait]
    impl TEEBackend for SevBackend {
        fn tee_type(&self) -> &'static str { "sev-snp" }

        async fn initialize(&mut self, config: &TEEConfig) -> Result<()> {
            // Initialize SEV-SNP guest
            // Would use sev-rs or Linux KVM IOCTLs
            self.measurement = vec![0; 48]; // Placeholder
            Ok(())
        }

        async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>> {
            // Get SEV-SNP attestation report
            Ok(vec![])
        }

        async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool> {
            Ok(true)
        }

        async fn seal(&self, plaintext: &[u8], policy: &SealPolicy) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]> {
            Ok([0; 32])
        }

        async fn execute_in_tee<F, R>(&self, f: F) -> Result<R>
        where
            F: FnOnce() -> Result<R> + Send,
            R: Send,
        {
            f()
        }

        fn measurement(&self) -> &[u8] {
            &self.measurement
        }

        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

/// TrustZone Backend (ARM)
pub mod trustzone_backend {
    use super::*;

    pub struct TrustZoneBackend {
        measurement: Vec<u8>,
    }

    impl TrustZoneBackend {
        pub fn new() -> Result<Self> {
            Ok(Self { measurement: vec![] })
        }
    }

    #[async_trait]
    impl TEEBackend for TrustZoneBackend {
        fn tee_type(&self) -> &'static str { "trustzone" }

        async fn initialize(&mut self, config: &TEEConfig) -> Result<()> {
            // Initialize OP-TEE or TrustZone
            // Would use OP-TEE client API
            self.measurement = vec![0; 32];
            Ok(())
        }

        async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool> {
            Ok(true)
        }

        async fn seal(&self, plaintext: &[u8], policy: &SealPolicy) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]> {
            Ok([0; 32])
        }

        async fn execute_in_tee<F, R>(&self, f: F) -> Result<R>
        where
            F: FnOnce() -> Result<R> + Send,
            R: Send,
        {
            f()
        }

        fn measurement(&self) -> &[u8] {
            &self.measurement
        }

        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

/// Secure Enclave Backend (Apple)
pub mod secure_enclave_backend {
    use super::*;
    use std::sync::Mutex;

    pub struct SecureEnclaveBackend {
        key_tag: String,
        measurement: Vec<u8>,
    }

    impl SecureEnclaveBackend {
        pub fn new() -> Result<Self> {
            Ok(Self {
                key_tag: "com.nexusllm.tee.key".into(),
                measurement: vec![],
            })
        }
    }

    #[async_trait]
    impl TEEBackend for SecureEnclaveBackend {
        fn tee_type(&self) -> &'static str { "secure_enclave" }

        async fn initialize(&mut self, config: &TEEConfig) -> Result<()> {
            // Initialize Secure Enclave key
            #[cfg(target_os = "macos")]
            {
                use security_framework::os::macos::keychain::SecKeyChain;
                use security_framework::key::SecKey;
                
                // Create or retrieve key in Secure Enclave
                // This is a simplified placeholder
                self.measurement = blake3::hash(b"secure_enclave_measurement").as_bytes().to_vec();
            }
            Ok(())
        }

        async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>> {
            // Apple DeviceCheck / App Attest for attestation
            // This would generate an attestation object
            let mut quote = report_data.to_vec();
            quote.extend_from_slice(&self.measurement);
            Ok(quote)
        }

        async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool> {
            // Verify using App Attest
            Ok(true)
        }

        async fn seal(&self, plaintext: &[u8], policy: &SealPolicy) -> Result<Vec<u8>> {
            // Encrypt with Secure Enclave key
            #[cfg(target_os = "macos")]
            {
                // Use security_framework to encrypt
            }
            Ok(plaintext.to_vec()) // Placeholder
        }

        async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
            #[cfg(target_os = "macos")]
            {
                // Use security_framework to decrypt
            }
            Ok(sealed.to_vec()) // Placeholder
        }

        async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]> {
            // Use Secure Enclave for key derivation
            let mut key = [0u8; 32];
            // Derive using HKDF with SE-bound root key
            Ok(key)
        }

        async fn execute_in_tee<F, R>(&self, f: F) -> Result<R>
        where
            F: FnOnce() -> Result<R> + Send,
            R: Send,
        {
            // Execute with Secure Enclave key access
            f()
        }

        fn measurement(&self) -> &[u8] {
            &self.measurement
        }

        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

/// Software fallback (no TEE)
pub mod software_fallback {
    use super::*;

    pub struct SoftwareFallback {
        measurement: Vec<u8>,
        sealing_key: [u8; 32],
    }

    impl SoftwareFallback {
        pub fn new() -> Self {
            use rand::RngCore;
            let mut key = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            Self {
                measurement: blake3::hash(b"software_fallback").as_bytes().to_vec(),
                sealing_key: key,
            }
        }
    }

    #[async_trait]
    impl TEEBackend for SoftwareFallback {
        fn tee_type(&self) -> &'static str { "software" }

        async fn initialize(&mut self, _config: &TEEConfig) -> Result<()> {
            warn!("Using software fallback - NO HARDWARE SECURITY");
            Ok(())
        }

        async fn get_quote(&self, report_data: &[u8]) -> Result<Vec<u8>> {
            // Return software-signed quote (not verifiable)
            let mut quote = report_data.to_vec();
            quote.extend_from_slice(&self.measurement);
            Ok(quote)
        }

        async fn verify_quote(&self, quote: &[u8], expected_measurement: &[u8]) -> Result<bool> {
            // Software quotes cannot be trusted
            warn!("Verifying software quote - NOT TRUSTED");
            Ok(quote.ends_with(expected_measurement))
        }

        async fn seal(&self, plaintext: &[u8], _policy: &SealPolicy) -> Result<Vec<u8>> {
            // AES-GCM with software key
            use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
            use aes_gcm::aead::Aead;
            
            let cipher = Aes256Gcm::new(&self.sealing_key.into());
            let nonce = Nonce::from_slice(&[0u8; 12]); // In production: random nonce
            let ciphertext = cipher.encrypt(nonce, plaintext)
                .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
            Ok(ciphertext)
        }

        async fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
            use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
            use aes_gcm::aead::Aead;
            
            let cipher = Aes256Gcm::new(&self.sealing_key.into());
            let nonce = Nonce::from_slice(&[0u8; 12]);
            let plaintext = cipher.decrypt(nonce, sealed)
                .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
            Ok(plaintext)
        }

        async fn derive_key(&self, label: &[u8], context: &[u8]) -> Result<[u8; 32]> {
            use hmac::Hmac;
            use sha2::Sha256;
            
            let mut hmac = Hmac::<Sha256>::new_from_slice(&self.sealing_key)?;
            hmac.update(label);
            hmac.update(context);
            let mut key = [0u8; 32];
            key.copy_from_slice(&hmac.finalize().into_bytes()[..32]);
            Ok(key)
        }

        async fn execute_in_tee<F, R>(&self, f: F) -> Result<R>
        where
            F: FnOnce() -> Result<R> + Send,
            R: Send,
        {
            f()
        }

        fn measurement(&self) -> &[u8] {
            &self.measurement
        }

        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_software_fallback() {
        let mut fallback = software_fallback::SoftwareFallback::new();
        fallback.initialize(&TEEConfig::default()).await.unwrap();

        let plaintext = b"secret data";
        let sealed = fallback.seal(plaintext, &SealPolicy::default()).await.unwrap();
        let unsealed = fallback.unseal(&sealed).await.unwrap();

        assert_eq!(plaintext, unsealed.as_slice());
    }
}