//! get_price: calls a public CoinGecko price endpoint (no auth key, no PII)
//! and returns a spot price stamped with T3N's cluster timestamp.

#[derive(serde::Deserialize)]
pub struct GetPriceReq {
    pub coin_id: String,
    pub vs_currency: String,
}

#[derive(serde::Serialize)]
pub struct GetPriceResp {
    pub coin_id: String,
    pub vs_currency: String,
    pub price: f64,
    pub fetched_at_secs: u64,
}

const COINGECKO_BASE: &str = "https://api.coingecko.com/api/v3";

/// Entry point called from `lib.rs`. `input` is the raw JSON bytes from the
/// node's `generic-input.input` field.
pub fn get_price(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: GetPriceReq = serde_json::from_slice(input)
        .map_err(|e| alloc::format!("get-price: bad input: {e}"))?;

    #[cfg(target_arch = "wasm32")]
    {
        let resp = get_price_wasm(req)?;
        serde_json::to_vec(&resp).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("get_price is only implemented on the wasm32 target".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
use crate::host::{
    interfaces::{http as http_iface, logging},
    tenant::tenant_context,
};

#[cfg(target_arch = "wasm32")]
fn get_price_wasm(req: GetPriceReq) -> Result<GetPriceResp, String> {
    let url = alloc::format!(
        "{COINGECKO_BASE}/simple/price?ids={}&vs_currencies={}",
        req.coin_id, req.vs_currency
    );

    let resp = http_iface::call(&http_iface::Request {
        method: http_iface::Verb::Get,
        url,
        // CoinGecko's public API 403s requests with no descriptive User-Agent
        // (returned bare "Accept: application/json" got rejected in testing).
        headers: Some(alloc::vec![
            ("Accept".to_string(), "application/json".to_string()),
            (
                "User-Agent".to_string(),
                "t3n-price-pulse-contract/0.1 (Terminal3 ADK bounty entry)".to_string(),
            ),
        ]),
        payload: None,
    })
    .map_err(|e| alloc::format!("coingecko call failed: {e}"))?;

    if resp.code != 200 {
        let body = alloc::string::String::from_utf8_lossy(&resp.payload);
        return Err(alloc::format!("coingecko HTTP {}: {body}", resp.code));
    }

    let body: serde_json::Value =
        serde_json::from_slice(&resp.payload).map_err(|e| e.to_string())?;

    let price = body[req.coin_id.as_str()][req.vs_currency.as_str()]
        .as_f64()
        .ok_or_else(|| alloc::format!("no price for {}/{}", req.coin_id, req.vs_currency))?;

    let _ = logging::info(&alloc::format!(
        "price-pulse: {} {} = {price}",
        req.coin_id, req.vs_currency
    ));

    Ok(GetPriceResp {
        coin_id: req.coin_id,
        vs_currency: req.vs_currency,
        price,
        fetched_at_secs: tenant_context::cluster_timestamp_secs(),
    })
}

#[derive(serde::Deserialize)]
pub struct GetPricesReq {
    pub coin_ids: Vec<String>,
    pub vs_currency: String,
}

#[derive(serde::Serialize)]
pub struct PriceEntry {
    pub coin_id: String,
    pub price: f64,
}

#[derive(serde::Serialize)]
pub struct GetPricesResp {
    pub vs_currency: String,
    pub prices: Vec<PriceEntry>,
    pub fetched_at_secs: u64,
}

/// Entry point called from `lib.rs`. Same envelope pattern as get_price.
pub fn get_prices(input: &[u8]) -> Result<Vec<u8>, String> {
    let req: GetPricesReq = serde_json::from_slice(input)
        .map_err(|e| alloc::format!("get-prices: bad input: {e}"))?;

    #[cfg(target_arch = "wasm32")]
    {
        let resp = get_prices_wasm(req)?;
        serde_json::to_vec(&resp).map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = req;
        Err("get_prices is only implemented on the wasm32 target".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn get_prices_wasm(req: GetPricesReq) -> Result<GetPricesResp, String> {
    if req.coin_ids.is_empty() {
        return Err("get-prices: coin_ids must not be empty".to_string());
    }

    // One outbound call for the whole watchlist — CoinGecko's `ids` param
    // takes a comma-separated list.
    let url = alloc::format!(
        "{COINGECKO_BASE}/simple/price?ids={}&vs_currencies={}",
        req.coin_ids.join(","),
        req.vs_currency
    );

    let resp = http_iface::call(&http_iface::Request {
        method: http_iface::Verb::Get,
        url,
        headers: Some(alloc::vec![
            ("Accept".to_string(), "application/json".to_string()),
            (
                "User-Agent".to_string(),
                "t3n-price-pulse-contract/0.1 (Terminal3 ADK bounty entry)".to_string(),
            ),
        ]),
        payload: None,
    })
    .map_err(|e| alloc::format!("coingecko call failed: {e}"))?;

    if resp.code != 200 {
        let body = alloc::string::String::from_utf8_lossy(&resp.payload);
        return Err(alloc::format!("coingecko HTTP {}: {body}", resp.code));
    }

    let body: serde_json::Value =
        serde_json::from_slice(&resp.payload).map_err(|e| e.to_string())?;

    let prices: Result<alloc::vec::Vec<PriceEntry>, alloc::string::String> = req
        .coin_ids
        .iter()
        .map(|coin_id| {
            let price = body[coin_id.as_str()][req.vs_currency.as_str()]
                .as_f64()
                .ok_or_else(|| alloc::format!("no price for {coin_id}/{}", req.vs_currency))?;
            Ok(PriceEntry {
                coin_id: coin_id.clone(),
                price,
            })
        })
        .collect();
    let prices = prices?;

    let _ = logging::info(&alloc::format!(
        "price-pulse: watchlist of {} coins fetched in one call",
        prices.len()
    ));

    Ok(GetPricesResp {
        vs_currency: req.vs_currency,
        prices,
        fetched_at_secs: tenant_context::cluster_timestamp_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_prices_non_wasm_returns_err() {
        let input = serde_json::to_vec(&serde_json::json!({
            "coin_ids": ["solana", "bitcoin"],
            "vs_currency": "usd",
        }))
        .unwrap();
        let result = get_prices(&input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("only implemented on the wasm32 target"));
    }

    #[test]
    fn get_prices_bad_input_returns_err() {
        let result = get_prices(b"not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad input"));
    }

    #[test]
    fn get_price_non_wasm_returns_err() {
        let input = serde_json::to_vec(&serde_json::json!({
            "coin_id": "solana",
            "vs_currency": "usd",
        }))
        .unwrap();
        let result = get_price(&input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("only implemented on the wasm32 target"));
    }

    #[test]
    fn get_price_bad_input_returns_err() {
        let result = get_price(b"not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad input"));
    }

    #[test]
    fn get_price_missing_field_returns_err() {
        let input = serde_json::to_vec(&serde_json::json!({ "coin_id": "solana" })).unwrap();
        let result = get_price(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bad input"));
    }
}
