use base64_url;
use std::convert::TryInto;
use veilid_core::CryptoKey;
use veilid_core::CryptoTyped;
use veilid_core::TypedKey;
use veilid_core::CRYPTO_KIND_VLD0;
use crate::error::AppResult;

pub fn create_veilid_cryptokey_from_base64(key_string: &str) -> AppResult<CryptoKey> {
    let key_vec = base64_url::decode(key_string)?;
    let key_array: [u8; 32] = key_vec.try_into()?;
    let crypto_key = CryptoKey::new(key_array);

    Ok(crypto_key)
}

pub fn create_veilid_typedkey_from_base64(key_string: &str) -> AppResult<CryptoTyped<CryptoKey>> {
    let crypto_key = create_veilid_cryptokey_from_base64(key_string)?;
    let typed_key = TypedKey::new(CRYPTO_KIND_VLD0, crypto_key);

    Ok(typed_key)
}