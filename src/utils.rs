use base64_url;
use std::convert::TryInto;
use veilid_core::RecordKey;
use veilid_core::CryptoTyped;
use veilid_core::CRYPTO_KIND_VLD0;
use crate::error::AppResult;

pub fn create_veilid_cryptokey_from_base64(key_string: &str) -> AppResult<RecordKey> {
    let key_vec = base64_url::decode(key_string)?;
    let key_array: [u8; 32] = key_vec.try_into()?;
    let record_key = RecordKey::new(key_array);

    Ok(record_key)
}

pub fn create_veilid_typedkey_from_base64(key_string: &str) -> AppResult<CryptoTyped<RecordKey>> {
    let record_key = create_veilid_cryptokey_from_base64(key_string)?;
    let typed_key = CryptoTyped::new(CRYPTO_KIND_VLD0, record_key);

    Ok(typed_key)
}