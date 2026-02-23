//! zkML Proof RPC Module
//!
//! This module provides RPC endpoints for:
//! - Submitting AI inference proofs
//! - Verifying zkML proofs
//! - Querying proof status
//! - Managing trusted model images
//!
//! SECURITY: State-changing operations require signature verification

use crate::helpers::{parse_address, verify_caller_signature};
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_core::Hash;
use luxtensor_zkvm::{
    ZkProver, ZkVerifier,
    ProofReceipt, ImageId, GuestInput, VerificationResult,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// ============================================================
// Types
// ============================================================

/// Proof submission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofSubmission {
    /// Task ID (from AI task dispatch)
    pub task_id: String,
    /// Image ID (model hash) - hex string
    pub image_id: String,
    /// Proof receipt serialized - hex string
    pub proof_receipt: String,
    /// Submitter address
    pub submitter: String,
}

/// Proof generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequest {
    /// Image ID (model hash)
    pub image_id: String,
    /// Input data (hex-encoded)
    pub input_data: String,
}

/// Stored proof info
#[derive(Debug, Clone)]
pub struct StoredProof {
    pub task_id: [u8; 32],
    pub image_id: ImageId,
    pub receipt_hash: Hash,
    pub is_verified: bool,
    pub submitter: [u8; 20],
    pub submitted_at: u64,
    pub verification_result: Option<VerificationResult>,
}

/// Context for zkML RPC
pub struct ZkmlRpcContext {
    pub prover: Arc<RwLock<ZkProver>>,
    pub verifier: Arc<RwLock<ZkVerifier>>,
    pub proofs: Arc<RwLock<HashMap<[u8; 32], StoredProof>>>,
    pub trusted_images: Arc<RwLock<HashMap<ImageId, TrustedModelInfo>>>,
}

/// Trusted model registration info
#[derive(Debug, Clone)]
pub struct TrustedModelInfo {
    pub image_id: ImageId,
    pub model_hash: Hash,
    pub name: String,
    pub registered_at: u64,
    pub registered_by: [u8; 20],
}

impl ZkmlRpcContext {
    /// Create a ZkmlRpcContext in **development mode** (accepts dev proofs).
    ///
    /// # Security Warning
    /// Dev mode accepts proofs without real cryptographic verification.
    /// **MUST NOT** be used for mainnet/testnet deployments.
    /// Use [`production()`](Self::production) instead.
    pub fn dev() -> Self {
        tracing::warn!(
            "ZkmlRpcContext created in DEV MODE — proofs are NOT cryptographically verified. \
             Use ZkmlRpcContext::production() for real deployments."
        );
        let prover = ZkProver::dev_prover();
        let verifier = ZkVerifier::dev_verifier();

        Self {
            prover: Arc::new(RwLock::new(prover)),
            verifier: Arc::new(RwLock::new(verifier)),
            proofs: Arc::new(RwLock::new(HashMap::new())),
            trusted_images: Arc::new(RwLock::new(HashMap::new())),
        }
    }



    /// Create with production config (real RISC Zero proofs)
    pub fn production() -> Self {
        let prover = ZkProver::default_prover();
        let verifier = ZkVerifier::default_verifier();

        Self {
            prover: Arc::new(RwLock::new(prover)),
            verifier: Arc::new(RwLock::new(verifier)),
            proofs: Arc::new(RwLock::new(HashMap::new())),
            trusted_images: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ZkmlRpcContext {
    /// Default uses **production mode** to prevent accidental dev-mode deployment.
    fn default() -> Self {
        Self::production()
    }
}

// ============================================================
// RPC Registration
// ============================================================

/// Register all zkML RPC methods
pub fn register_zkml_methods(ctx: Arc<ZkmlRpcContext>, io: &mut IoHandler) {
    register_proof_submission_methods(&ctx, io);
    register_proof_verification_methods(&ctx, io);
    register_model_management_methods(&ctx, io);
}

fn register_proof_submission_methods(ctx: &Arc<ZkmlRpcContext>, io: &mut IoHandler) {
    let proofs = ctx.proofs.clone();
    let verifier = ctx.verifier.clone();

    // zkml_submitProof - Submit a zkML proof
    io.add_method("zkml_submitProof", move |params: Params| {
        let proofs = proofs.clone();
        let verifier = verifier.clone();
        async move {
        let submission: ProofSubmission = params.parse()?;

        // Parse task_id
        let task_id = parse_hash32(&submission.task_id)?;

        // Parse image_id
        let image_id_bytes = parse_hash32(&submission.image_id)?;
        let image_id = ImageId::new(image_id_bytes);

        // Parse proof receipt
        let receipt_bytes = hex::decode(submission.proof_receipt.trim_start_matches("0x"))
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid proof receipt hex"))?;

        let receipt: ProofReceipt = ProofReceipt::from_bytes(&receipt_bytes)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid proof receipt format"))?;

        // Parse submitter
        let submitter = parse_address(&submission.submitter)?;
        let submitter_bytes: [u8; 20] = *submitter.as_bytes();

        // Verify the proof
        let verification_result = {
            let ver = verifier.read();
            ver.verify(&receipt)
                .map_err(|e| jsonrpc_core::Error::invalid_params(format!("Verification error: {}", e)))?
        };

        // Compute receipt hash
        let receipt_hash = receipt.commitment_hash();

        // Store the proof
        let stored_proof = StoredProof {
            task_id,
            image_id,
            receipt_hash,
            is_verified: verification_result.is_valid,
            submitter: submitter_bytes,
            submitted_at: current_timestamp(),
            verification_result: Some(verification_result.clone()),
        };

        {
            let mut proofs_map = proofs.write();
            proofs_map.insert(task_id, stored_proof);
        }

        info!(
            "Proof submitted: task={} verified={}",
            hex::encode(&task_id[..8]),
            verification_result.is_valid
        );

        Ok(serde_json::json!({
            "success": true,
            "task_id": format!("0x{}", hex::encode(task_id)),
            "receipt_hash": format!("0x{}", hex::encode(receipt_hash)),
            "is_verified": verification_result.is_valid,
            "verification_time_us": verification_result.verification_time_us,
            "error": verification_result.error,
        }))
        }
    });

    let proofs = ctx.proofs.clone();

    // zkml_getProof - Get proof status
    io.add_method("zkml_getProof", move |params: Params| {
        let proofs = proofs.clone();
        async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing task ID"));
        }

        let task_id = parse_hash32(&parsed[0])?;

        let proofs_map = proofs.read();
        if let Some(proof) = proofs_map.get(&task_id) {
            Ok(serde_json::json!({
                "task_id": format!("0x{}", hex::encode(proof.task_id)),
                "image_id": format!("0x{}", hex::encode(proof.image_id.as_bytes())),
                "receipt_hash": format!("0x{}", hex::encode(proof.receipt_hash)),
                "is_verified": proof.is_verified,
                "submitter": format!("0x{}", hex::encode(proof.submitter)),
                "submitted_at": proof.submitted_at,
                "verification_error": proof.verification_result.as_ref()
                    .and_then(|r| r.error.clone()),
            }))
        } else {
            Ok(Value::Null)
        }
        }
    });
}

fn register_proof_verification_methods(ctx: &Arc<ZkmlRpcContext>, io: &mut IoHandler) {
    let verifier = ctx.verifier.clone();

    // zkml_verifyProof - Verify a proof without storing
    io.add_method("zkml_verifyProof", move |params: Params| {
        let verifier = verifier.clone();
        async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing proof receipt"));
        }

        // Parse proof receipt
        let receipt_bytes = hex::decode(parsed[0].trim_start_matches("0x"))
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid proof receipt hex"))?;

        let receipt: ProofReceipt = ProofReceipt::from_bytes(&receipt_bytes)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid proof receipt format"))?;

        // Verify
        let result = {
            let ver = verifier.read();
            ver.verify(&receipt)
                .map_err(|e| jsonrpc_core::Error::invalid_params(format!("Verification error: {}", e)))?
        };

        Ok(serde_json::json!({
            "is_valid": result.is_valid,
            "image_id": format!("0x{}", hex::encode(result.image_id.as_bytes())),
            "journal_hash": format!("0x{}", hex::encode(result.journal_hash)),
            "verification_time_us": result.verification_time_us,
            "error": result.error,
        }))
        }
    });

    let prover = ctx.prover.clone();

    // zkml_generateProof - Generate a proof (development/testing)
    io.add_method("zkml_generateProof", move |params: Params| {
        let prover = prover.clone();
        async move {
        let req: ProofRequest = params.parse()?;

        // Parse image_id
        let image_id_bytes = parse_hash32(&req.image_id)?;
        let image_id = ImageId::new(image_id_bytes);

        // Parse input data
        let input_bytes = hex::decode(req.input_data.trim_start_matches("0x"))
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid input data hex"))?;

        let guest_input = GuestInput::new(input_bytes);

        // Generate proof - prove is async, need to block
        let receipt = {
            let prov = prover.read();
            // Use Handle::current() to run async in sync context
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    prov.prove(image_id, guest_input).await
                })
            })
            .map_err(|e| jsonrpc_core::Error::invalid_params(format!("Proving error: {}", e)))?
        };

        // Serialize receipt
        let receipt_bytes = receipt.to_bytes()
            .map_err(|e| jsonrpc_core::Error::invalid_params(format!("Serialization error: {}", e)))?;

        info!(
            "Proof generated: image={} cycles={} time={}ms",
            hex::encode(&image_id_bytes[..8]),
            receipt.metadata.cycles,
            receipt.metadata.proving_time_ms
        );

        Ok(serde_json::json!({
            "success": true,
            "image_id": format!("0x{}", hex::encode(image_id_bytes)),
            "receipt": format!("0x{}", hex::encode(&receipt_bytes)),
            "commitment_hash": format!("0x{}", hex::encode(receipt.commitment_hash())),
            "cycles": receipt.metadata.cycles,
            "proving_time_ms": receipt.metadata.proving_time_ms,
            "gpu_used": receipt.metadata.gpu_used,
        }))
        }
    });
}

fn register_model_management_methods(ctx: &Arc<ZkmlRpcContext>, io: &mut IoHandler) {
    let trusted_images = ctx.trusted_images.clone();
    let verifier = ctx.verifier.clone();

    // zkml_registerModel - Register a trusted model
    // SECURITY: Now requires signature verification
    io.add_method("zkml_registerModel", move |params: Params| {
        let trusted_images = trusted_images.clone();
        let verifier = verifier.clone();
        async move {
        #[derive(Deserialize)]
        struct RegisterRequest {
            image_id: String,
            model_hash: String,
            name: String,
            registrar: String,
            signature: String,  // Required for authentication
        }

        let req: RegisterRequest = params.parse()?;

        let image_id_bytes = parse_hash32(&req.image_id)?;
        let image_id = ImageId::new(image_id_bytes);

        let model_hash = parse_hash32(&req.model_hash)?;
        let registrar = parse_address(&req.registrar)?;
        let registrar_bytes: [u8; 20] = *registrar.as_bytes();

        // Security: Verify signature ownership
        let message = format!(
            "register_model:{}:{}:{}",
            hex::encode(&image_id_bytes),
            hex::encode(&model_hash),
            req.name
        );

        let sig_valid = verify_caller_signature(&registrar, &message, &req.signature, 0)
            .or_else(|_| verify_caller_signature(&registrar, &message, &req.signature, 1));

        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed - caller does not own registrar address"
            ));
        }

        let info = TrustedModelInfo {
            image_id,
            model_hash,
            name: req.name.clone(),
            registered_at: current_timestamp(),
            registered_by: registrar_bytes,
        };

        // Add to trusted images in verifier
        {
            let mut ver = verifier.write();
            ver.trust_image(image_id);
        }

        // Store info
        {
            let mut images = trusted_images.write();
            images.insert(image_id, info);
        }

        info!("Model registered (verified): {} ({})", req.name, hex::encode(&image_id_bytes[..8]));

        Ok(serde_json::json!({
            "success": true,
            "image_id": format!("0x{}", hex::encode(image_id_bytes)),
            "name": req.name,
            "message": "Model registered (signature verified)"
        }))
        }
    });

    let trusted_images = ctx.trusted_images.clone();

    // zkml_listTrustedModels - List all trusted models
    io.add_method("zkml_listTrustedModels", move |_params: Params| {
        let trusted_images = trusted_images.clone();
        async move {
        let images = trusted_images.read();
        let list: Vec<serde_json::Value> = images
            .values()
            .map(|info| {
                serde_json::json!({
                    "image_id": format!("0x{}", hex::encode(info.image_id.as_bytes())),
                    "model_hash": format!("0x{}", hex::encode(info.model_hash)),
                    "name": info.name,
                    "registered_at": info.registered_at,
                    "registered_by": format!("0x{}", hex::encode(info.registered_by)),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "models": list,
            "count": list.len(),
        }))
        }
    });

    let trusted_images = ctx.trusted_images.clone();

    // zkml_isModelTrusted - Check if model is trusted
    io.add_method("zkml_isModelTrusted", move |params: Params| {
        let trusted_images = trusted_images.clone();
        async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing image ID"));
        }

        let image_id_bytes = parse_hash32(&parsed[0])?;
        let image_id = ImageId::new(image_id_bytes);

        let images = trusted_images.read();
        let is_trusted = images.contains_key(&image_id);

        let info = images.get(&image_id);

        Ok(serde_json::json!({
            "is_trusted": is_trusted,
            "name": info.map(|i| i.name.clone()),
            "registered_at": info.map(|i| i.registered_at),
        }))
        }
    });
}

// ============================================================
// Helper Functions
// ============================================================

fn parse_hash32(hex_str: &str) -> Result<[u8; 32], jsonrpc_core::Error> {
    let hex_str = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(hex_str)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex string"))?;

    if bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params("Hash must be 32 bytes"));
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = ZkmlRpcContext::dev();
        assert!(ctx.proofs.read().is_empty());
        assert!(ctx.trusted_images.read().is_empty());
    }

    #[test]
    fn test_parse_hash32() {
        let hash = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = parse_hash32(hash).unwrap();
        assert_eq!(result[31], 1);
    }
}
