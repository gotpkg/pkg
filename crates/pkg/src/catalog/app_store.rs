//! The App Store, through Apple's iTunes Search API: the catalog behind `mas:` packages.

use serde::Deserialize;

use super::Client;
use crate::failure::Failure;

/// One app as the App Store describes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub track_id: u64,
    pub track_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub seller_name: Option<String>,
    #[serde(default)]
    pub track_view_url: Option<String>,
    #[serde(default)]
    pub formatted_price: Option<String>,
}

#[derive(Deserialize)]
struct Results {
    #[serde(default)]
    results: Vec<App>,
}

pub struct AppStore<'a> {
    client: &'a Client,
    api: &'a str,
}

impl<'a> AppStore<'a> {
    pub fn new(client: &'a Client, api: &'a str) -> AppStore<'a> {
        AppStore { client, api }
    }

    /// The apps with these App Store IDs that the store still sells.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when the App Store cannot be reached.
    pub fn lookup(&self, ids: &[&str]) -> Result<Vec<App>, Failure> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids.join(",");
        let results: Option<Results> = self
            .client
            .get_json(&format!("{}/lookup", self.api), &[("id", &ids), ("entity", MAC)])?;
        Ok(results.map(|results| results.results).unwrap_or_default())
    }

    /// Mac apps matching `term`, in the App Store's order.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when the App Store cannot be reached.
    pub fn search(&self, term: &str) -> Result<Vec<App>, Failure> {
        let results: Option<Results> = self.client.get_json(
            &format!("{}/search", self.api),
            &[("term", term), ("entity", MAC), ("limit", "25")],
        )?;
        Ok(results.map(|results| results.results).unwrap_or_default())
    }
}

/// The iTunes entity for macOS apps. Without it, a universal app reports its iPad version.
const MAC: &str = "desktopSoftware";
