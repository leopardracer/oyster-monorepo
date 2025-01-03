use clap::Parser;
use http_on_vsock_client::vsock_client::vsock_connector;
use hyper::{body::Body, Request, Uri};
use serde_json::json;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// url to query
    #[clap(short, long, value_parser)]
    url: String,

    /// owner address
    #[clap(short, long, value_parser)]
    owner_address: String,

    /// gas key
    #[clap(short, long, value_parser)]
    gas_key: String,

    /// ws api key
    #[clap(short, long, value_parser)]
    ws_api_key: String,

    /// list of chain ids
    #[clap(short, long, value_parser)]
    chain_ids: Vec<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // WARN: Had to do Box::pin to get it to work, vsock_connector is not Unpin for some reason
    let connector = tower::service_fn(|dst: Uri| Box::pin(vsock_connector(dst)));

    let client = hyper::Client::builder().build::<_, Body>(connector);

    while let Err(err) = client.get(cli.url.clone().try_into()?).await {
        if err.is_connect() {
            println!("{:?}", err);
            println!("Connection refused, retrying...");
            // sleep for 1 second
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }
        return Err(err.into());
    }

    // prepare the post request
    let body = json!({
        "owner_address_hex": cli.owner_address.clone(),
    })
    .to_string();
    let request = Request::post(cli.url.clone() + "immutable-config")
        .header("Content-Type", "application/json")
        .body(Body::from(body))?;
    let mut resp = client.request(request).await?;
    println!("{:?}", resp.status());
    println!(
        "{:?}",
        String::from_utf8(hyper::body::to_bytes(resp.into_body()).await?.to_vec())?
    );

    // set mutable config
    let body = json!({
        "gas_key_hex": cli.gas_key.clone(),
        "ws_api_key": cli.ws_api_key.clone(),
    })
    .to_string();
    let request = Request::post(cli.url.clone() + "mutable-config")
        .header("Content-Type", "application/json")
        .body(Body::from(body))?;
    resp = client.request(request).await?;
    println!("{:?}", resp.status());
    println!(
        "{:?}",
        String::from_utf8(hyper::body::to_bytes(resp.into_body()).await?.to_vec())?
    );

    // Get the config
    resp = client
        .get((cli.url.clone() + "gateway-details").try_into()?)
        .await?;
    let body_bytes = hyper::body::to_bytes(resp.into_body()).await?;
    let body_str = String::from_utf8(body_bytes.to_vec())?;
    let json: serde_json::Value = serde_json::from_str(&body_str)?;
    println!("{:?}", json);

    // Start the executor
    let body = json!({
        "chain_ids": cli.chain_ids.clone(),
    })
    .to_string();
    println!("{:?}", body);
    let request = Request::post(cli.url.clone() + "signed-registration-message")
        .header("Content-Type", "application/json")
        .body(Body::from(body))?;
    resp = client.request(request).await?;
    println!("{:?}", resp);
    let body_bytes = hyper::body::to_bytes(resp.into_body()).await?;
    let body_str = String::from_utf8(body_bytes.to_vec())?;
    let json: serde_json::Value = serde_json::from_str(&body_str)?;
    println!("{:?}", json);

    Ok(())
}
