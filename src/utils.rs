use base64_url;
use std::convert::TryInto;
use veilid_core::{RecordKey, BareRecordKey, BareOpaqueRecordKey, CRYPTO_KIND_VLD0};
use crate::error::AppResult;

/// Parse a RecordKey from either:
/// - Typed format: "VLD0:base64_key:base64_hash" (from RecordKey::to_string())
/// - Raw base64 format: "base64_key" (legacy)
pub fn create_veilid_cryptokey_from_base64(key_string: &str) -> AppResult<RecordKey> {
    // Check if this is the typed format (contains colons)
    if let Some((_, bare_input)) = key_string.split_once(':') {
        // BareRecordKey::try_decode expects "key:hash" (2 parts).
        // RecordKey::to_string() produces "KIND:key:hash" (3 parts), so drop the first segment.
        let bare_key = BareRecordKey::try_decode(bare_input)
            .map_err(|e| anyhow::anyhow!("Invalid record key encoding: {e}"))?;
        return Ok(RecordKey::new(CRYPTO_KIND_VLD0, bare_key));
    }

    // Legacy: raw base64 format
    let key_vec = base64_url::decode(key_string)?;
    let key_array: [u8; 32] = key_vec.try_into().map_err(|_| {
        anyhow::anyhow!("Invalid key length: expected 32 bytes")
    })?;
    let record_key = RecordKey::new(
        CRYPTO_KIND_VLD0,
        BareRecordKey::new(BareOpaqueRecordKey::from(&key_array[..]), None)
    );

    Ok(record_key)
}
