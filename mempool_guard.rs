use reqwest::Client;
use serde_json::{json, Value};
use std::{env, time::Duration};

async fn rpc(client: &Client, url: &str, method: &str, params: Value) -> anyhow::Result<Value> {
    let res = client
        .post(url)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await?;
    let j: Value = res.json().await?;
    if let Some(err) = j.get("error") {
        anyhow::bail!("{}", err)
    }
    Ok(j["result"].clone())
}

fn hex_to_u64(h: &str) -> u64 {
    u64::from_str_radix(h.trim_start_matches("0x"), 16).unwrap_or(0)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = env::var("RPC_URL").expect("Set RPC_URL to your JSON-RPC endpoint");
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let mut pending: u64 = 0;
    if let Ok(v) = rpc(&client, &url, "txpool_status", json!([])).await {
        if let Some(p) = v.get("pending").and_then(|x| x.as_str()) {
            pending = hex_to_u64(p);
        }
    }

    let gas_price_hex = rpc(&client, &url, "eth_gasPrice", json!([])).await?
        .as_str().unwrap_or("0x0").to_string();
    let base_fee_hex = rpc(&client, &url, "eth_getBlockByNumber", json!(["latest", false])).await?
        .get("baseFeePerGas").and_then(|x| x.as_str()).unwrap_or("0x0").to_string();

    let gas_price = u128::from_str_radix(gas_price_hex.trim_start_matches("0x"), 16).unwrap_or(0);
    let base_fee = u128::from_str_radix(base_fee_hex.trim_start_matches("0x"), 16).unwrap_or(0);

    let pending_w = 0.001f64;
    let fee_w = 1e-20f64;
    let score = (pending as f64) * pending_w + ((gas_price as i128 - base_fee as i128).max(0) as f64) * fee_w;

    println!("pending_tx_estimate: {}", pending);
    println!("gas_price_wei: {}", gas_price);
    println!("base_fee_wei: {}", base_fee);
    println!("anomaly_score: {:.4}", score);

    let threshold: f64 = env::var("MP_THRESHOLD_SCORE").ok().and_then(|s| s.parse().ok()).unwrap_or(25.0);
    if score > threshold {
        eprintln!("ALERT: anomaly_score {:.2} > {}", score, threshold);
        std::process::exit(1);
    }
    Ok(())
}
