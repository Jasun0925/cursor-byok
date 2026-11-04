//! Implements the configured external search provider adapter.
use std::sync::Arc;

use semble_core::{ContentType, FindRelatedRequest, SearchEngine, SearchRequest, SembleConfig};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::{store::Store, Error, Result};

static ENGINE: OnceCell<Arc<SearchEngine>> = OnceCell::const_new();

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContentSelection {
    #[default]
    Code,
    Docs,
    Config,
    All,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchArguments {
    query: String,
    repo: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
    #[serde(default = "default_snippet_lines")]
    max_snippet_lines: Option<usize>,
    #[serde(default)]
    content: ContentSelection,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FindRelatedArguments {
    repo: String,
    file_path: String,
    line: usize,
    #[serde(default = "default_top_k")]
    top_k: usize,
    #[serde(default = "default_snippet_lines")]
    max_snippet_lines: Option<usize>,
    #[serde(default)]
    content: ContentSelection,
}

pub(crate) enum Operation {
    Search(SearchArguments),
    FindRelated(FindRelatedArguments),
}

impl Operation {
    pub(crate) fn parse(tool_name: &str, arguments: Value) -> std::result::Result<Self, String> {
        let operation = match tool_name {
            "semblesearch" => Operation::Search(
                serde_json::from_value(arguments).map_err(|error| error.to_string())?,
            ),
            "semblefindrelated" => Operation::FindRelated(
                serde_json::from_value(arguments).map_err(|error| error.to_string())?,
            ),
            _ => return Err(format!("unsupported Semble tool: {tool_name}")),
        };
        let (top_k, invalid) = match &operation {
            Self::Search(a) => (a.top_k, a.query.trim().is_empty()),
            Self::FindRelated(a) => (a.top_k, a.line == 0),
        };
        if invalid || top_k == 0 || top_k > 100 {
            return Err("Invalid Semble query, line, or top_k".into());
        }
        Ok(operation)
    }
    pub(crate) fn repo(&self) -> &str {
        match self {
            Self::Search(a) => &a.repo,
            Self::FindRelated(a) => &a.repo,
        }
    }
    pub(crate) fn content(&self) -> Vec<ContentType> {
        content(match self {
            Self::Search(a) => a.content,
            Self::FindRelated(a) => a.content,
        })
    }
    pub(crate) async fn execute_snapshot(
        self,
        files: Vec<semble_core::source::SnapshotFile>,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
        store: Option<Store>,
    ) -> std::result::Result<Value, String> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Search was cancelled".into());
        }
        let engine = engine(store).await.map_err(|error| error.to_string())?;
        tokio::task::spawn_blocking(move || match self {
            Self::Search(a) => engine
                .search_snapshot(
                    SearchRequest {
                        query: a.query,
                        repo: a.repo.into(),
                        top_k: a.top_k,
                        max_snippet_lines: a.max_snippet_lines,
                        content: content(a.content),
                    },
                    &files,
                    &cancelled,
                )
                .and_then(json_value),
            Self::FindRelated(a) => engine
                .find_related_snapshot(
                    FindRelatedRequest {
                        repo: a.repo.into(),
                        file_path: a.file_path,
                        line: a.line,
                        top_k: a.top_k,
                        max_snippet_lines: a.max_snippet_lines,
                        content: content(a.content),
                    },
                    &files,
                    &cancelled,
                )
                .and_then(json_value),
        })
        .await
        .map_err(|error| format!("Semble search worker failed: {error}"))?
        .map_err(|error| error.to_string())
    }
}

pub(crate) async fn execute(
    tool_name: &str,
    arguments: Value,
    store: Option<Store>,
) -> std::result::Result<Value, String> {
    let operation = Operation::parse(tool_name, arguments)?;
    let engine = engine(store).await.map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || match operation {
        Operation::Search(arguments) => engine
            .search(SearchRequest {
                query: arguments.query,
                repo: arguments.repo.into(),
                top_k: arguments.top_k,
                max_snippet_lines: arguments.max_snippet_lines,
                content: content(arguments.content),
            })
            .and_then(json_value),
        Operation::FindRelated(arguments) => engine
            .find_related(FindRelatedRequest {
                repo: arguments.repo.into(),
                file_path: arguments.file_path,
                line: arguments.line,
                top_k: arguments.top_k,
                max_snippet_lines: arguments.max_snippet_lines,
                content: content(arguments.content),
            })
            .and_then(json_value),
    })
    .await
    .map_err(|error| format!("Semble search worker failed: {error}"))?
    .map_err(|error| error.to_string())
}

async fn engine(store: Option<Store>) -> Result<Arc<SearchEngine>> {
    ENGINE
        .get_or_try_init(|| async move {
            let builder = match store {
                Some(store) => crate::network::blocking_client_builder(&store).await?,
                None => reqwest::blocking::Client::builder().use_native_tls(),
            };
            tokio::task::spawn_blocking(move || {
                let client = builder.build()?;
                SearchEngine::load_default_with_client(SembleConfig::default(), &client)
                    .map(Arc::new)
                    .map_err(|error| Error::Config(format!("load Semble search engine: {error}")))
            })
            .await
            .map_err(|error| Error::Config(format!("load Semble search engine: {error}")))?
        })
        .await
        .cloned()
}

fn json_value(response: semble_core::SearchResponse) -> semble_core::Result<Value> {
    serde_json::to_value(response)
        .map_err(|error| semble_core::Error::Serialization(error.to_string()))
}

fn content(selection: ContentSelection) -> Vec<ContentType> {
    match selection {
        ContentSelection::Code => vec![ContentType::Code],
        ContentSelection::Docs => vec![ContentType::Docs],
        ContentSelection::Config => vec![ContentType::Config],
        ContentSelection::All => vec![ContentType::Code, ContentType::Docs, ContentType::Config],
    }
}

fn default_top_k() -> usize {
    5
}

fn default_snippet_lines() -> Option<usize> {
    Some(10)
}
