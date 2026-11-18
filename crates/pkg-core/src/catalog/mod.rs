//! The catalogs pkg searches and describes packages from: Homebrew's formula and cask API and the
//! App Store's search API. mise's registry comes from mise itself.

pub(crate) mod app_store;
pub(crate) mod homebrew;

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::time::Duration;

use serde::de::DeserializeOwned;
use ureq::{Agent, Proxy};

use crate::error::{Error as Failure, ErrorDetails, Recovery};

/// An HTTPS client shared by the catalogs.
pub(crate) struct Client {
    agent: Agent,
}

impl Client {
    pub(crate) fn new(environment: &BTreeMap<OsString, OsString>) -> Result<Client, Failure> {
        let agent = Agent::config_builder()
            .proxy(proxy(environment)?)
            .timeout_global(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            .user_agent(concat!("pkg/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Ok(Client { agent })
    }
}

impl Client {
    /// GETs `url` with `query` and parses the JSON body, or `None` for a 404.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when the catalog cannot be reached, answers with another error status, or
    /// sends something that is not the expected JSON.
    pub(crate) fn get_json<T: DeserializeOwned>(
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
        .with_details(ErrorDetails { url: Some(url.to_string()), ..Default::default() })
        .recover(Recovery::RetryNetwork)
}

fn proxy(environment: &BTreeMap<OsString, OsString>) -> Result<Option<Proxy>, Failure> {
    let get = |key: &str| environment.get(OsStr::new(key)).and_then(|value| value.to_str());
    let proxy =
        ["ALL_PROXY", "all_proxy", "HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"]
            .iter()
            .filter_map(|key| get(key))
            .find_map(|value| Proxy::new(value).ok());
    let Some(proxy) = proxy else {
        return Ok(None);
    };
    let Some(exclusions) = get("NO_PROXY").or_else(|| get("no_proxy")) else {
        return Ok(Some(proxy));
    };
    let mut builder = Proxy::builder(proxy.protocol())
        .host(proxy.host())
        .port(proxy.port())
        .resolve_target(proxy.resolve_target())
        .no_proxy(exclusions);
    if let Some(username) = proxy.username() {
        builder = builder.username(username);
    }
    if let Some(password) = proxy.password() {
        builder = builder.password(password);
    }
    builder
        .build()
        .map(Some)
        .map_err(|_| Failure::precondition("cannot configure the catalog proxy"))
}
