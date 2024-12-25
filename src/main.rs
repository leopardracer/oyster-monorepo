mod api_impl;
mod chain_util;
mod common_chain_gateway_state_service;
mod common_chain_interaction;
mod config;
mod constant;
mod contract_abi;
mod error;
mod job_subscription_management;
mod model;

#[cfg(test)]
mod test_util;

use actix_web::web::Data;
use actix_web::{App, HttpServer};
use alloy::primitives::Address;
use alloy::signers::k256::ecdsa::SigningKey;
use alloy::signers::utils::public_key_to_address;
use alloy::transports::http::reqwest::Url;
use anyhow::Context;
use clap::Parser;
use env_logger::Env;
use std::collections::HashSet;
use std::error::Error;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use tokio::fs;

use crate::api_impl::{
    export_signed_registration_message, get_gateway_details, index, inject_immutable_config,
    inject_mutable_config,
};
use crate::model::{AppState, ConfigManager};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(
        long,
        value_parser,
        default_value = "./oyster_serverless_gateway_config.json"
    )]
    config_file: String,
    #[clap(long, value_parser, default_value = "6001")]
    port: u16,
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    // Load the configuration file
    let args = Cli::parse();
    let config_manager = ConfigManager::new(&args.config_file);
    let config = config_manager.load_config().unwrap();

    match Url::parse(&config.common_chain_http_url) {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Invalid common chain http url: {}", err);
            return Err(err.into());
        }
    }

    match Url::parse(&config.common_chain_ws_url) {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Invalid common chain ws url: {}", err);
            return Err(err.into());
        }
    }

    let enclave_signer_key: SigningKey = SigningKey::from_slice(
        fs::read(config.enclave_secret_key)
            .await
            .context("Failed to read the enclave signer key")?
            .as_slice(),
    )
    .context("Invalid enclave signer key")?;

    let enclave_address = public_key_to_address(&enclave_signer_key.verifying_key());

    // Create a Appstate
    let app_data = Data::new(AppState {
        enclave_signer_key,
        enclave_address,
        wallet: Arc::new(RwLock::new(String::new())),
        common_chain_id: config.common_chain_id,
        common_chain_http_url: config.common_chain_http_url,
        common_chain_ws_url: config.common_chain_ws_url,
        gateways_contract_addr: config.gateways_contract_addr,
        gateway_jobs_contract_addr: config.gateway_jobs_contract_addr,
        request_chain_ids: Mutex::new(HashSet::new()),
        registered: Arc::new(AtomicBool::new(false)),
        epoch: config.epoch,
        time_interval: config.time_interval,
        offset_for_epoch: config.offset_for_epoch,
        enclave_owner: Mutex::new(Address::ZERO),
        immutable_params_injected: Mutex::new(false),
        mutable_params_injected: Arc::new(AtomicBool::new(false)),
        registration_events_listener_active: Mutex::new(false),
        contracts_client: Mutex::new(None),
    });

    // Start a http server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(index)
            .service(inject_immutable_config)
            .service(inject_mutable_config)
            .service(export_signed_registration_message)
            .service(get_gateway_details)
    })
    .bind(("0.0.0.0", args.port))
    .context(format!("could not bind to port {}", args.port))?
    .run();

    println!("Node server started on port {}", args.port);

    server.await?;

    Ok(())
}
