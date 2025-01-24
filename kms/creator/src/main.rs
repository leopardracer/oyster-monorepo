use alloy::signers::local::PrivateKeySigner;
use axum::{extract::State, http::StatusCode};
use nucypher_core::{ferveo::api::DkgPublicKey, Conditions};
use rand::{rngs::OsRng, RngCore};

#[derive(Clone)]
struct AppState {
    signer: PrivateKeySigner,
    conditions: Conditions,
    dkg_public_key: DkgPublicKey,
}

// generate new randomness and encyrpt it against the DKG key
pub async fn generate(State(state): State<AppState>) -> (StatusCode, String) {
    // check if randomness already exists
    let guard = state.randomness.lock().unwrap();
    if guard.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            "randomness already exists\n".into(),
        );
    }
    drop(guard);

    // generate randomness
    let mut randomness = [0u8; 64];
    OsRng.fill_bytes(randomness.as_mut());

    // generate encrypted message
    let Ok(encrypted) = crate::taco::encrypt(
        &randomness,
        &state.conditions,
        state.dkg_public_key,
        state.signer,
    ) else {
        // NOTE: Explicitly do not do anything with the error message
        // lest it leaks something about the encryption process
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to encrypt\n".into(),
        );
    };

    // set randomness and encrypted
    let mut randomness_guard = state.randomness.lock().unwrap();
    let mut encrypted_guard = state.encrypted.lock().unwrap();
    *randomness_guard = Some(randomness);
    *encrypted_guard = encrypted.clone();
    drop(encrypted_guard);
    drop(randomness_guard);

    (StatusCode::OK, encrypted)
}

fn main() {
    println!("Hello, world!");
}
