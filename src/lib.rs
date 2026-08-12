#![warn(clippy::style, missing_debug_implementations)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

extern crate alloc;

pub const CONTRACT_VERSION: &str = "0.1.2";

wit_bindgen::generate!({
    world: "price-pulse",
    path: "wit",
    additional_derives: [
        serde::Deserialize,
        serde::Serialize,
    ],
    generate_all,
});

mod price;

struct Component;

#[cfg(target_arch = "wasm32")]
impl exports::z::price_pulse::contracts::Guest for Component {
    fn get_price(
        req: exports::z::price_pulse::contracts::GenericInput,
    ) -> Result<alloc::vec::Vec<u8>, alloc::string::String> {
        let input = req.input.ok_or("get-price: missing input")?;
        price::get_price(&input)
    }

    fn get_prices(
        req: exports::z::price_pulse::contracts::GenericInput,
    ) -> Result<alloc::vec::Vec<u8>, alloc::string::String> {
        let input = req.input.ok_or("get-prices: missing input")?;
        price::get_prices(&input)
    }
}

#[cfg(target_arch = "wasm32")]
export!(Component);

#[cfg(test)]
mod tests {
    use super::CONTRACT_VERSION;

    #[test]
    fn contract_version_is_semver() {
        let parts: Vec<&str> = CONTRACT_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "CONTRACT_VERSION must be MAJOR.MINOR.PATCH");
        for part in parts {
            assert!(part.parse::<u32>().is_ok(), "each part must be a number");
        }
    }
}
