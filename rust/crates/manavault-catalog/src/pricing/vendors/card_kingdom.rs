//! Card Kingdom's official singles pricelist
//! (`Manavault.Pricing.Vendors.CardKingdom`). One request returns every
//! single with its Scryfall id, NM retail price, and foil flag; etched
//! printings are flagged through the variation text.

use std::time::Duration;

use async_trait::async_trait;
use lotus::Finish;
use serde::Deserialize;
use serde_json::Value;

use super::{Vendor, VendorFeed, VendorRow, get};
use crate::pricing::money;
use manavault_core::state::AppState;

pub const PRICELIST_URL: &str = "https://api.cardkingdom.com/api/v2/pricelist";

pub struct CardKingdom {
    pub url: String,
}

#[async_trait]
impl VendorFeed for CardKingdom {
    fn vendor(&self) -> Vendor {
        Vendor::CardKingdom
    }

    async fn fetch(&self, state: &AppState) -> Result<Vec<VendorRow>, String> {
        let (status, body) = get(state, &self.url, Duration::from_secs(300)).await?;
        if status != reqwest::StatusCode::OK {
            return Err(format!("Card Kingdom returned HTTP {}", status.as_u16()));
        }
        // Card Kingdom serves the JSON with a text/html content type; the
        // body is decoded whatever the header says.
        Ok(rows(&body))
    }
}

/// The pricelist, reduced to the fields prices are read from. Decoding the
/// ~70 MB feed into a generic JSON tree would allocate over half a gigabyte
/// for fields that are never read; unknown fields are skipped instead.
/// Fields stay loosely typed so an odd value skips its product, not the
/// whole feed.
#[derive(Deserialize)]
struct Pricelist {
    #[serde(default)]
    data: Vec<Product>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Product {
    scryfall_id: Value,
    variation: Value,
    is_foil: Value,
    price_retail: Value,
}

/// Maps the pricelist body into finish-keyed rows; anything unexpected is
/// skipped.
#[must_use]
pub fn rows(body: &[u8]) -> Vec<VendorRow> {
    let Ok(pricelist) = serde_json::from_slice::<Pricelist>(body) else {
        return Vec::new();
    };
    pricelist
        .data
        .iter()
        .filter_map(|product| {
            let scryfall_id = product.scryfall_id.as_str().filter(|id| !id.is_empty())?;
            let cents = money::to_cents(&product.price_retail)?;
            Some(VendorRow::new(scryfall_id, finish(product), cents))
        })
        .collect()
}

fn finish(product: &Product) -> Finish {
    let variation = match &product.variation {
        Value::Null => String::new(),
        Value::String(text) => text.to_lowercase(),
        other => other.to_string().to_lowercase(),
    };
    if variation.contains("etched") {
        Finish::Etched
    } else if product.is_foil == Value::Bool(true) || product.is_foil.as_str() == Some("true") {
        Finish::Foil
    } else {
        Finish::Nonfoil
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_products_to_finish_keyed_rows() {
        let body = json!({"data": [
            {"scryfall_id": "aaa", "variation": "", "is_foil": "false", "price_retail": "0.35"},
            {"scryfall_id": "bbb", "variation": "", "is_foil": "true", "price_retail": "1.25"},
            {"scryfall_id": "ccc", "variation": "Foil Etched", "is_foil": "true", "price_retail": "9.99"},
            {"scryfall_id": "", "variation": "", "is_foil": "false", "price_retail": "1.00"},
            {"scryfall_id": "ddd", "variation": "", "is_foil": "false", "price_retail": "0.00"},
            {"scryfall_id": "eee", "is_foil": true, "price_retail": 2}
        ]});
        assert_eq!(
            rows(body.to_string().as_bytes()),
            vec![
                VendorRow::new("aaa", Finish::Nonfoil, 35),
                VendorRow::new("bbb", Finish::Foil, 125),
                VendorRow::new("ccc", Finish::Etched, 999),
                VendorRow::new("eee", Finish::Foil, 200),
            ]
        );
    }

    #[test]
    fn tolerates_unexpected_payloads() {
        assert_eq!(rows(b"{}").len(), 0);
        assert_eq!(rows(b"\"nope\"").len(), 0);
        assert_eq!(rows(b"not json").len(), 0);
    }
}
