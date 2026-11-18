//! Package search across Homebrew's formulae and casks, mise's registry, and the App Store.

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog;
use crate::catalog::app_store::AppStore;
use crate::catalog::homebrew::{Homebrew, IndexEntry};
use crate::common::Machine;
use crate::matching::{self, Page};
use crate::{Client, Error, Outcome, Progress, SearchRequest, SearchSource};

/// One package that matches the query.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Found {
    /// What to pass to `pkg install`.
    pub package: String,
    /// `brew`, `brew-cask`, `tool`, or `mas`.
    pub manager: String,
    /// The display name: a cask's or app's title, else the package's name.
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    /// Whether your mise config already declares it.
    pub declared: bool,
    /// Relevance; higher is better.
    pub score: f64,
}

impl Client {
    /// Ranks the chosen catalogs' packages against the query's words, best first; equal scores
    /// keep the source order Homebrew formulae, casks, mise tools, then App Store apps. Empty
    /// `managers` searches every source. Homebrew's index is cached for a day; a stale copy
    /// answers with a warning when Homebrew cannot be reached. An unreachable App Store becomes
    /// a warning unless it is the only source requested.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a cursor this operation did not return; `Unavailable` when a required catalog cannot be
    /// reached; `PreconditionFailed` when mise cannot run or a configuration file cannot be
    /// read; and `MiseFailed` when mise cannot answer.
    pub fn search(
        &mut self,
        request: SearchRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<Page<Found>>, Error> {
        let terms = &request.terms;
        let chosen = &request.managers;
        let wants = |source: SearchSource| chosen.is_empty() || chosen.contains(&source);
        let machine = Machine::load(&self.settings)?;
        let environment = &machine.environment;
        let client = catalog::Client::new(&self.settings.child_env)?;
        let phrase = request.query.trim().to_lowercase();
        let mut found: Vec<Found> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        if wants(SearchSource::Brew) || wants(SearchSource::Cask) {
            let loaded =
                Homebrew::new(&client, environment.homebrew_api, environment.cache_dir).index()?;
            warnings.extend(loaded.warning);
            for (source, entries) in [
                (SearchSource::Brew, &loaded.index.formulae),
                (SearchSource::Cask, &loaded.index.casks),
            ] {
                if wants(source) {
                    found.extend(entries.iter().filter_map(|entry| {
                        homebrew_match(&machine, source.as_str(), entry, terms, &phrase)
                    }));
                }
            }
        }
        if wants(SearchSource::Tool) {
            for tool in environment.mise.registry()? {
                let others: Vec<&str> = tool.aliases.iter().map(String::as_str).collect();
                let description = tool.description.clone().unwrap_or_default();
                let score = score(terms, &phrase, &tool.short, &others, &description);
                if score > 0.0 {
                    let declared = machine.declarations.tool_file(&tool.short).is_some();
                    found.push(Found {
                        package: tool.short.clone(),
                        manager: SearchSource::Tool.as_str().into(),
                        name: tool.short,
                        description,
                        version: None,
                        declared,
                        score,
                    });
                }
            }
        }
        if wants(SearchSource::AppStore) {
            match AppStore::new(&client, environment.app_store_api).search(&request.query) {
                Ok(apps) => found.extend(apps.into_iter().filter_map(|app| {
                    let description = app.description.unwrap_or_default();
                    let summary: String = description.chars().take(300).collect();
                    let score = score(terms, &phrase, &app.track_name, &[], &summary);
                    let package = format!("mas:{}", app.track_id);
                    (score > 0.0).then(|| Found {
                        declared: machine.declarations.package_file(&package).is_some(),
                        package,
                        manager: SearchSource::AppStore.as_str().into(),
                        name: app.track_name,
                        description: summary,
                        version: app.version,
                        score,
                    })
                })),
                Err(failure) if chosen.len() != 1 => {
                    warnings.push(format!("skipped the App Store: {}", failure.message));
                }
                Err(failure) => return Err(failure),
            }
        }

        // Stable, so equal scores keep the source order above.
        found.sort_by(|left, right| right.score.total_cmp(&left.score));
        for item in &mut found {
            item.score = matching::rounded(item.score);
        }
        let page = matching::page(found, request.cursor.as_deref(), request.limit)?;
        Ok(Outcome::with_warnings(page, warnings, progress))
    }
}

fn homebrew_match(
    machine: &Machine<'_>,
    manager: &str,
    entry: &IndexEntry,
    terms: &[String],
    phrase: &str,
) -> Option<Found> {
    let mut others: Vec<&str> = entry.aliases.iter().map(String::as_str).collect();
    others.extend(entry.title.as_deref());
    let score = score(terms, phrase, &entry.name, &others, &entry.description);
    (score > 0.0).then(|| {
        let package = format!("{manager}:{}", entry.name);
        Found {
            declared: machine.declarations.package_file(&package).is_some(),
            package,
            manager: manager.to_string(),
            name: entry.title.clone().unwrap_or_else(|| entry.name.clone()),
            description: entry.description.clone(),
            version: (!entry.version.is_empty()).then(|| entry.version.clone()),
            score,
        }
    })
}

/// How well a package matches: the share of query words found in its names (full weight, or 0.6
/// for a name that starts with the word) or its description (half), plus 1 when a name is the
/// whole query and 0.5 when one starts with it. Matches that miss more than half the query words
/// score 0.
fn score(terms: &[String], phrase: &str, name: &str, others: &[&str], description: &str) -> f64 {
    let spellings: Vec<String> =
        std::iter::once(name).chain(others.iter().copied()).map(str::to_lowercase).collect();
    let mut names: Vec<String> = Vec::new();
    for spelling in &spellings {
        names.extend(matching::words(spelling));
    }
    let described = matching::words(description);
    let matched: Vec<f64> = terms
        .iter()
        .map(|term| {
            let prefix = if spellings.iter().any(|spelling| spelling.starts_with(term.as_str())) {
                0.6
            } else {
                0.0
            };
            matching::best_match(term, &names)
                .max(prefix)
                .max(matching::best_match(term, &described) * 0.5)
        })
        .collect();
    let hits = matched.iter().filter(|score| **score > 0.0).count();
    if hits * 2 < terms.len() || hits == 0 {
        return 0.0;
    }
    let base =
        matched.iter().sum::<f64>() / f64::from(u32::try_from(terms.len()).unwrap_or(u32::MAX));
    let hyphenated = phrase.replace(' ', "-");
    let bonus = if spellings.iter().any(|spelling| *spelling == phrase || *spelling == hyphenated) {
        1.0
    } else if spellings.iter().any(|spelling| spelling.starts_with(&hyphenated)) {
        0.5
    } else {
        0.0
    };
    base + bonus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(query: &str, candidates: &[(&str, &str)]) -> Vec<String> {
        let terms = matching::query_words(query);
        let mut scored: Vec<(f64, &str)> = candidates
            .iter()
            .map(|(name, description)| (score(&terms, query, name, &[], description), *name))
            .filter(|(score, _)| *score > 0.0)
            .collect();
        scored.sort_by(|left, right| right.0.total_cmp(&left.0));
        scored.into_iter().map(|(_, name)| name.to_string()).collect()
    }

    #[test]
    fn an_exact_name_outranks_a_description_mention() {
        let candidates = [
            ("ffmpegthumbnailer", "Create thumbnails for your video files"),
            ("mpv", "Media player based on MPlayer and mplayer2, uses ffmpeg"),
            ("ffmpeg", "Play, record, convert, and stream audio and video"),
            ("wget", "Internet file retriever"),
        ];
        assert_eq!(rank("ffmpeg", &candidates), ["ffmpeg", "ffmpegthumbnailer", "mpv"]);
    }

    #[test]
    fn several_words_must_mostly_match() {
        let candidates = [
            ("iina", "Free and open-source media player"),
            ("vlc", "Multimedia player"),
            ("transmit", "File transfer"),
        ];
        assert_eq!(rank("media player", &candidates), ["iina", "vlc"]);
    }
}
