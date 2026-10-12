//! `ManaPool`'s public singles price feed
//! (`Manavault.Pricing.Vendors.ManaPool`). Each printing and finish uses the
//! lowest near-mint listing, falling back to the lowest
//! lightly-played-or-better listing and then the lowest listing of any
//! condition. Listing prices are keyed per finish, so nonfoil, foil, and
//! etched never share a price.

use std::time::Duration;

use async_trait::async_trait;
use lotus::Finish;
use serde::Deserialize;
use serde_json::Value;

use super::{Vendor, VendorFeed, VendorRow, get};
use manavault_core::state::AppState;

pub const PRICES_URL: &str = "https://manapool.com/api/v1/prices/singles";

pub struct ManaPool {
    pub url: String,
}

#[async_trait]
impl VendorFeed for ManaPool {
    fn vendor(&self) -> Vendor {
        Vendor::ManaPool
    }

    async fn fetch(&self, state: &AppState) -> Result<Vec<VendorRow>, String> {
        let (status, body) = get(state, &self.url, Duration::from_secs(300)).await?;
        if status != reqwest::StatusCode::OK {
            return Err(format!("ManaPool returned HTTP {}", status.as_u16()));
        }
        Ok(rows(&body))
    }
}

/// The feed, reduced to the fields prices are read from. Decoding the
/// ~50 MB feed into a generic JSON tree would allocate several hundred
/// megabytes for fields that are never read; unknown fields are skipped
/// instead. Fields stay loosely typed so an odd value skips its price, not
/// the whole feed.
#[derive(Deserialize)]
struct Feed {
    #[serde(default)]
    data: Vec<Variant>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Variant {
    scryfall_id: Value,
    price_cents: Value,
    price_cents_lp_plus: Value,
    price_cents_nm: Value,
    price_cents_foil: Value,
    price_cents_lp_plus_foil: Value,
    price_cents_nm_foil: Value,
    price_cents_etched: Value,
    price_cents_lp_plus_etched: Value,
    price_cents_nm_etched: Value,
}

impl Variant {
    /// Each finish's listing prices, best condition first: near mint, then
    /// lightly played or better, then any condition.
    fn listing_prices(&self) -> [(Finish, [&Value; 3]); 3] {
        [
            (
                Finish::Nonfoil,
                [
                    &self.price_cents_nm,
                    &self.price_cents_lp_plus,
                    &self.price_cents,
                ],
            ),
            (
                Finish::Foil,
                [
                    &self.price_cents_nm_foil,
                    &self.price_cents_lp_plus_foil,
                    &self.price_cents_foil,
                ],
            ),
            (
                Finish::Etched,
                [
                    &self.price_cents_nm_etched,
                    &self.price_cents_lp_plus_etched,
                    &self.price_cents_etched,
                ],
            ),
        ]
    }
}

/// Maps the feed body into listing rows; market prices are never used.
#[must_use]
pub fn rows(body: &[u8]) -> Vec<VendorRow> {
    let Ok(feed) = serde_json::from_slice::<Feed>(body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for variant in &feed.data {
        let Some(id) = variant.scryfall_id.as_str().filter(|id| !id.is_empty()) else {
            continue;
        };
        for (finish, prices) in variant.listing_prices() {
            if let Some(cents) = prices
                .into_iter()
                .find_map(|price| price.as_i64().filter(|cents| *cents > 0))
            {
                out.push(VendorRow::new(id, finish, cents));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_app::TestApp;

    #[test]
    fn uses_the_lowest_near_mint_listing_for_each_finish() {
        let body = json!({"data": [{
            "scryfall_id": "aaa", "price_market": 500, "price_market_foil": 750,
            "price_cents": 410, "price_cents_lp_plus": 450, "price_cents_nm": 525,
            "price_cents_foil": 600, "price_cents_lp_plus_foil": 700, "price_cents_nm_foil": 790,
            "price_cents_etched": 850, "price_cents_lp_plus_etched": 875, "price_cents_nm_etched": 900
        }]});
        assert_eq!(
            rows(body.to_string().as_bytes()),
            vec![
                VendorRow::new("aaa", Finish::Nonfoil, 525),
                VendorRow::new("aaa", Finish::Foil, 790),
                VendorRow::new("aaa", Finish::Etched, 900),
            ]
        );
    }

    #[test]
    fn falls_back_per_finish_and_never_borrows_another_finish() {
        let body = json!({"data": [
            {"scryfall_id": "aaa", "price_market": 999, "price_cents": 147, "price_cents_lp_plus": 290,
             "price_cents_nm": null, "price_cents_foil": 310, "price_cents_lp_plus_foil": null, "price_cents_nm_foil": null},
            {"scryfall_id": "foil-only", "price_market": 8595, "price_market_foil": 20955, "price_cents_nm_foil": 43000},
            {"scryfall_id": "nonfoil-only", "price_market_foil": 300, "price_cents_nm": 200}
        ]});
        assert_eq!(
            rows(body.to_string().as_bytes()),
            vec![
                VendorRow::new("aaa", Finish::Nonfoil, 290),
                VendorRow::new("aaa", Finish::Foil, 310),
                VendorRow::new("foil-only", Finish::Foil, 43_000),
                VendorRow::new("nonfoil-only", Finish::Nonfoil, 200),
            ]
        );
    }

    #[test]
    fn skips_missing_and_invalid_listing_prices() {
        let body = json!({"data": [
            {"scryfall_id": "aaa", "price_market": 500, "price_market_foil": 750},
            {"scryfall_id": "bbb", "price_cents_nm": 0, "price_cents_foil": -100},
            {"scryfall_id": "ccc", "price_cents_nm": "12.00"},
            {"scryfall_id": "ddd", "price_cents_nm": 12.5},
            {"scryfall_id": "", "price_cents_nm": 200},
            {"scryfall_id": "ddd", "low_price": 400},
            {"scryfall_id": "aaa"}
        ]});
        assert_eq!(rows(body.to_string().as_bytes()).len(), 0);
        assert_eq!(rows(b"{}").len(), 0);
        assert_eq!(rows(b"[1, 2]").len(), 0);
    }

    #[tokio::test]
    async fn fetch_maps_the_feed_into_listing_rows() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [{
                "scryfall_id": "aaa", "price_market": 8595, "price_cents_lp_plus": 7000, "price_cents_nm_foil": 43000
            }]})))
            .mount(&server)
            .await;
        let app = TestApp::new().await;
        let feed = ManaPool { url: server.uri() };
        assert_eq!(
            feed.fetch(&app.state).await,
            Ok(vec![
                VendorRow::new("aaa", Finish::Nonfoil, 7000),
                VendorRow::new("aaa", Finish::Foil, 43_000),
            ])
        );
        let failing = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&failing)
            .await;
        let feed = ManaPool { url: failing.uri() };
        assert_eq!(
            feed.fetch(&app.state).await,
            Err("ManaPool returned HTTP 503".to_owned())
        );
    }
}
