use crate::{
    commands::job::types::{ListArgs, Relay},
    configs::global::{ARBITRUM_ONE_RPC_URL, INDEXER_URL, RELAY_CONTRACT_ADDRESS},
    utils::conversion::iso8601_to_epoch,
};
use alloy::{primitives::U256, providers::ProviderBuilder};
use anyhow::{Context, Result};
use prettytable::{row, Table};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Serialize, Debug)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value
}

#[derive(Deserialize, Debug)]
struct JobNode {
    #[serde(rename = "txHash")]
    tx_hash: String,
    #[serde(rename = "startTime")]
    start_time: String,
    status: String,
}

#[derive(Deserialize, Debug)]
struct JobEdge {
    node: JobNode,
}

#[derive(Deserialize, Debug)]
struct AllJobs {
    #[serde(rename = "totalCount")]
    total_count: i32,
    edges: Vec<JobEdge>,
}

#[derive(Deserialize, Debug)]
struct GraphQLResponse {
    data: Option<serde_json::Value>,
    errors: Option<Vec<serde_json::Value>>,
}

fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs() as i64
}

fn determine_job_status(start_time_epoch: i64, overall_timeout: U256) -> String {
    let current_time = get_current_timestamp();
    let timeout_seconds = overall_timeout.to_string().parse::<i64>().unwrap_or(0);

    if current_time - (start_time_epoch + timeout_seconds) > 0 {
        "CANCELLABLE".to_string()
    } else {
        "PENDING".to_string()
    }
}

/// List pending jobs for a given address
pub async fn list_jobs(args: ListArgs) -> Result<()> {
    // Create provider with no wallet (read-only)
    let rpc_url = ARBITRUM_ONE_RPC_URL
        .parse()
        .context("Failed to parse RPC URL")?;
    let provider = ProviderBuilder::new().on_http(rpc_url);

    // Parse addresses
    let contract_address = RELAY_CONTRACT_ADDRESS
        .parse()
        .context("Failed to parse relay contract address")?;

    // Create contract instance
    let contract = Relay::new(contract_address, provider.clone());

    let overall_timeout = contract
        .OVERALL_TIMEOUT()
        .call()
        .await
        .context("Failed to fetch overall timeout")?
        ._0;

    let client = reqwest::Client::new();

    let query = json!({
        "query": r#"
            query($address: String!, $status: String!) {
                allJobs(
                    filter: {
                        jobOwner: { equalToInsensitive: $address },
                        status: { equalTo: $status }
                    }
                ) {
                    totalCount
                    edges {
                        node {
                            status
                            txHash
                            startTime
                            jobOwner
                        }
                    }
                }
            }
        "#,
        "variables": {
            "address": args.address.to_string(),
            "status": args.status.to_string()
        }
    });

    let request_body = GraphQLRequest {
        query: query["query"].as_str().unwrap().to_string(),
        variables: query["variables"].clone()
    };

    let response = client
        .post(INDEXER_URL)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let response_data: GraphQLResponse = response.json().await?;

    if let Some(errors) = response_data.errors {
        anyhow::bail!("GraphQL query failed: {:?}", errors);
    }

    let mut table = Table::new();
    table.add_row(row!["Transaction hash", "Creation time", "Status"]);

    if let Some(data) = response_data.data {
        let all_jobs: AllJobs = serde_json::from_value(data["allJobs"].clone())?;

        info!(
            "Found {} jobs with status {}",
            all_jobs.total_count, args.status
        );

        for edge in all_jobs.edges {
            let job = edge.node;
            let status = if args.status.to_string() == "pending" {
                let start_time_in_epoch = iso8601_to_epoch(&job.start_time)
                    .context("Failed to convert start time to epoch")?;
                determine_job_status(start_time_in_epoch, overall_timeout)
            } else {
                job.status.to_uppercase()
            };

            table.add_row(row![job.tx_hash, job.start_time, status]);
        }
        if all_jobs.total_count > 0 {
            table.printstd();
        }
    }

    Ok(())
}
