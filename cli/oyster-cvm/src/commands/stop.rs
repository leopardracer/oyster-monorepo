use crate::configs::global::{ARBITRUM_ONE_RPC_URL, OYSTER_MARKET_ADDRESS};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, B256},
    providers::{Provider, ProviderBuilder, WalletProvider},
    signers::local::PrivateKeySigner,
    sol,
};
use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    OysterMarket,
    "src/abis/oyster_market_abi.json"
);

pub async fn stop_oyster_instance(job_id: &str, wallet_private_key: &str) -> Result<()> {
    info!("Stopping oyster instance with:");
    info!("  Job ID: {}", job_id);

    // Setup wallet and provider with signer
    let private_key = FixedBytes::<32>::from_slice(&hex::decode(wallet_private_key)?);
    let signer = PrivateKeySigner::from_bytes(&private_key)?;
    let wallet = EthereumWallet::from(signer);

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(
            ARBITRUM_ONE_RPC_URL
                .parse()
                .context("Failed to parse RPC URL")?,
        );
    info!(
        "Signer address: {:?}",
        provider
            .signer_addresses()
            .next()
            .ok_or_else(|| anyhow!("No signer address found"))?
    );

    // Create contract instance
    let market_address = OYSTER_MARKET_ADDRESS
        .parse()
        .context("Failed to parse market address")?;
    let market = OysterMarket::new(market_address, provider);

    // Parse job ID once
    let job_id_bytes = job_id.parse::<B256>().context("Failed to parse job ID")?;

    // Check if job exists
    let job = market
        .jobs(job_id_bytes)
        .call()
        .await
        .context("Failed to fetch job details")?;
    if job.owner == Address::ZERO {
        return Err(anyhow!("Job {} does not exist", job_id));
    }

    // First, set the job's rate to 0 using the jobReviseRateInitiate call.
    info!("Found job, initiating rate update to 0...");
    let revise_send_result = market
        .jobReviseRateInitiate(job_id_bytes, alloy::primitives::U256::from(0))
        .send()
        .await;
    let revise_tx_hash = match revise_send_result {
        Ok(tx_call_result) => tx_call_result
            .watch()
            .await
            .context("Failed to get transaction hash for rate revise")?,
        Err(err) => {
            return Err(anyhow!("Failed to send rate revise transaction: {:?}", err));
        }
    };

    info!("Rate revise transaction sent: {:?}", revise_tx_hash);

    // Verify the revise transaction execution.
    let revise_receipt = market
        .provider()
        .get_transaction_receipt(revise_tx_hash)
        .await
        .context("Failed to get transaction receipt for rate revise")?
        .ok_or_else(|| anyhow!("Rate revise transaction receipt not found"))?;
    if !revise_receipt.status() {
        return Err(anyhow!(
            "Rate revise transaction failed - check contract interaction"
        ));
    }

    info!("Job rate updated successfully to 0!");

    // Wait for 5 minutes before closing the job.
    info!("Waiting for 5 minutes before closing the job...");
    sleep(Duration::from_secs(300)).await;

    // Check if job is already closed before attempting to close
    let job = market
        .jobs(job_id_bytes)
        .call()
        .await
        .context("Failed to fetch job details")?;

    if job.owner == Address::ZERO {
        info!("Job is already closed!");
        return Ok(());
    }

    // Only proceed with closing if job still exists
    info!("Initiating job close...");
    let send_result = market.jobClose(job_id_bytes).send().await;
    let tx_hash = match send_result {
        Ok(tx_call_result) => tx_call_result
            .watch()
            .await
            .context("Failed to get transaction hash for job close")?,
        Err(err) => {
            return Err(anyhow!("Failed to send stop transaction: {:?}", err));
        }
    };

    info!("Stop transaction sent: {:?}", tx_hash);

    // Verify jobClose transaction success.
    let receipt = market
        .provider()
        .get_transaction_receipt(tx_hash)
        .await
        .context("Failed to get transaction receipt for job close")?
        .ok_or_else(|| anyhow!("Job close transaction receipt not found"))?;

    if !receipt.status() {
        return Err(anyhow!(
            "Job close transaction failed - check contract interaction"
        ));
    }

    info!("Instance stopped successfully!");
    Ok(())
}
