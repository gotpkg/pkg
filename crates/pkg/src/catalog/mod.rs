//! The catalogs pkg searches and describes packages from: Homebrew's formula and cask API and the
//! App Store's search API. mise's registry comes from mise itself.

pub mod app_store;
pub mod homebrew;

use std::io::Read;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;
use ureq::Agent;

use crate::failure::Failure;

/// An HTTPS client shared by the catalogs.
pub struct Client {
    agent: Agent,
}

impl Default for Client {
    fn default() -> Client {
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            .user_agent(concat!("pkg/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Client { agent }
    }
}

impl Client {
    /// GETs `url` with `query` and parses the JSON body, or `None` for a 404.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when the catalog cannot be reached, answers with another error status, or
    /// sends something that is not the expected JSON.
    pub fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Option<T>, Failure> {
        let mut request = self.agent.get(url);
        for (key, value) in query {
            request = request.query(*key, *value);
        }
        let response = request.call().map_err(|error| unreachable(url, &error.to_string()))?;
        let status = response.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(unreachable(url, &format!("it answered {status}")));
        }
        let reader = std::io::BufReader::new(response.into_body().into_reader());
        read_json(reader).map(Some).map_err(|error| unreachable(url, &error))
    }
}

fn read_json<T: DeserializeOwned>(reader: impl Read) -> Result<T, String> {
    serde_json::from_reader(reader)
        .map_err(|error| format!("its answer is not what pkg expects: {error}"))
}

fn unreachable(url: &str, problem: &str) -> Failure {
    let host = url.split('/').nth(2).unwrap_or(url);
    Failure::unavailable(format!("cannot reach {host}: {problem}"))
        .with_details(json!({ "url": url }))
        .suggest("Check the network connection and retry the same command")
}
