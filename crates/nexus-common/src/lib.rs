//! Shared types and utilities for NexusLLM

pub use nexus_proto::*;

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum NexusError {
        #[error("Configuration error: {0}")]
        Config(String),

        #[error("Network error: {0}")]
        Network(String),

        #[error("Execution error: {0}")]
        Execution(String),

        #[error("TEE error: {0}")]
        TEE(String),

        #[error("Attestation error: {0}")]
        Attestation(String),

        #[error("Serialization error: {0}")]
        Serialization(#[from] serde_json::Error),

        #[error("Protobuf error: {0}")]
        Protobuf(#[from] prost::DecodeError),

        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("Cryptographic error: {0}")]
        Crypto(String),

        #[error("Not found: {0}")]
        NotFound(String),

        #[error("Invalid argument: {0}")]
        InvalidArgument(String),

        #[error("Internal error: {0}")]
        Internal(String),
    }

    pub type Result<T> = std::result::Result<T, NexusError>;
}

pub mod crypto {
    use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
    use rand::rngs::OsRng;
    use rand::RngCore;

    /// Generate a new Ed25519 keypair
    pub fn generate_keypair() -> (SigningKey, VerifyingKey) {
        let mut csprng = OsRng;
        let mut bytes = [0u8; 32];
        csprng.fill_bytes(&mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    /// Sign a message
    pub fn sign(signing_key: &SigningKey, message: &[u8]) -> Signature {
        signing_key.sign(message)
    }

    /// Verify a signature
    pub fn verify(verifying_key: &VerifyingKey, message: &[u8], signature: &Signature) -> bool {
        verifying_key.verify(message, signature).is_ok()
    }

    /// Derive a key from a seed using HKDF
    pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
        use hmac::{Hmac, Mac};
        use sha2::{Sha256, digest::KeyInit};
        let mut hmac = <Hmac<Sha256> as KeyInit>::new_from_slice(salt).unwrap();
        hmac.update(ikm);
        hmac.finalize().into_bytes()[..32].try_into().unwrap()
    }

    /// HKDF expand
    pub fn hkdf_expand(prk: &[u8; 32], info: &[u8], output: &mut [u8]) {
        use hmac::{Hmac, Mac};
        use sha2::{Sha256, digest::KeyInit};
        let mut hmac = <Hmac<Sha256> as KeyInit>::new_from_slice(prk).unwrap();
        hmac.update(info);
        hmac.update(&[1u8]);
        output.copy_from_slice(&hmac.finalize().into_bytes()[..output.len()]);
    }
}

pub mod serialization {
    use prost::Message;

    pub fn encode_to_vec<T: Message>(msg: &T) -> Vec<u8> {
        let mut buf = Vec::with_capacity(msg.encoded_len());
        msg.encode(&mut buf).unwrap();
        buf
    }

    pub fn decode_from_slice<T: Message + Default>(buf: &[u8]) -> Result<T, prost::DecodeError> {
        T::decode(buf)
    }
}

pub mod metrics {
    use metrics::{counter, gauge, histogram};

    pub fn init_metrics() {
        // In metrics 0.22+, registration is automatic on first use
    }

    pub fn record_task_success(duration: f64, ttft: f64, tpot: f64) {
        counter!("nexus_tasks_total").increment(1);
        histogram!("nexus_task_duration_seconds").record(duration);
        histogram!("nexus_ttft_seconds").record(ttft);
        histogram!("nexus_tpot_seconds").record(tpot);
    }

    pub fn record_task_failure() {
        counter!("nexus_tasks_failed_total").increment(1);
    }

    pub fn record_kv_transfer(bytes: u64) {
        histogram!("nexus_kv_transfer_bytes").record(bytes as f64);
    }
}

pub mod time {
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn now_nanos() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}

pub mod tensor {
    use candle_core::{Device, Tensor, DType, Result as CandleResult};

    pub fn zeros(shape: &[usize], dtype: DType, device: &Device) -> CandleResult<Tensor> {
        Tensor::zeros(shape, dtype, device)
    }

    pub fn ones(shape: &[usize], dtype: DType, device: &Device) -> CandleResult<Tensor> {
        Tensor::ones(shape, dtype, device)
    }

    pub fn from_vec(data: Vec<f32>, shape: &[usize], device: &Device) -> CandleResult<Tensor> {
        Tensor::from_vec(data, shape, device)
    }

    pub fn matmul(a: &Tensor, b: &Tensor) -> CandleResult<Tensor> {
        a.matmul(b)
    }

    pub fn softmax(t: &Tensor, dim: usize) -> CandleResult<Tensor> {
        candle_nn::ops::softmax(t, dim)
    }

    pub fn layer_norm(t: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> CandleResult<Tensor> {
        candle_nn::ops::layer_norm(t, weight, bias, eps)
    }
}