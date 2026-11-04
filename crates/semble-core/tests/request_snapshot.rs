use std::{
    fs,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

use semble_core::{
    source::SnapshotFile, Embedder, Error, FindRelatedRequest, SearchEngine, SearchRequest,
    SembleConfig,
};

struct ProbeEmbedder {
    calls: Arc<AtomicUsize>,
    cancel: Option<Arc<AtomicBool>>,
}

impl Embedder for ProbeEmbedder {
    fn id(&self) -> &str {
        "request-snapshot-probe"
    }
    fn dimensions(&self) -> usize {
        2
    }
    fn encode(&self, texts: &[String]) -> semble_core::Result<Vec<Vec<f32>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
    }
}

fn file(path: &str, content: &str) -> SnapshotFile {
    SnapshotFile {
        relative_path: path.into(),
        bytes: content.as_bytes().to_vec(),
    }
}

#[test]
fn snapshots_with_the_same_workspace_path_never_share_files_or_disk_indexes() {
    let cache = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("lib.rs"), "fn local_secret() {}\n").unwrap();
    let engine = SearchEngine::with_embedder(
        SembleConfig::new(cache.path()),
        Arc::new(ProbeEmbedder {
            calls: Arc::new(AtomicUsize::new(0)),
            cancel: None,
        }),
    );
    let running = AtomicBool::new(false);
    for (symbol, content) in [
        ("host_alpha", "fn host_alpha() { /* alpha only */ }\n"),
        ("host_beta", "fn host_beta() { /* beta only */ }\n"),
    ] {
        let result = engine
            .search_snapshot(
                SearchRequest::new(symbol, source.path()),
                &[file("lib.rs", content)],
                &running,
            )
            .unwrap();
        assert_eq!(result.results.len(), 1);
        assert_eq!(
            result.results[0].content.as_deref(),
            Some(content.trim_end())
        );
    }
    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
}

#[test]
fn case_distinct_remote_files_and_related_search_do_not_depend_on_local_filesystem() {
    let cache = tempfile::tempdir().unwrap();
    let engine = SearchEngine::with_embedder(
        SembleConfig::new(cache.path()),
        Arc::new(ProbeEmbedder {
            calls: Arc::new(AtomicUsize::new(0)),
            cancel: None,
        }),
    );
    let running = AtomicBool::new(false);
    let files = [
        file("Case.rs", "fn upper_case() {}\n"),
        file("case.rs", "fn lower_case() {}\n"),
    ];
    for (query, path) in [("upper_case", "Case.rs"), ("lower_case", "case.rs")] {
        let result = engine
            .search_snapshot(SearchRequest::new(query, "/remote/only"), &files, &running)
            .unwrap();
        assert_eq!(result.results[0].file_path, path);
    }
    let result = engine
        .find_related_snapshot(
            FindRelatedRequest {
                repo: "/remote/only".into(),
                file_path: "Case.rs".into(),
                line: 1,
                top_k: 5,
                max_snippet_lines: None,
                content: vec![],
            },
            &files,
            &running,
        )
        .unwrap();
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].file_path, "case.rs");
    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
}

#[test]
fn cancellation_prevents_embedding_or_stops_before_the_next_file() {
    let cache = tempfile::tempdir().unwrap();
    let cancelled = Arc::new(AtomicBool::new(true));
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = SearchEngine::with_embedder(
        SembleConfig::new(cache.path()),
        Arc::new(ProbeEmbedder {
            calls: calls.clone(),
            cancel: Some(cancelled.clone()),
        }),
    );
    let files = [
        file("one.rs", "fn one() {}\n"),
        file("two.rs", "fn two() {}\n"),
    ];
    let search = || {
        engine.search_snapshot(
            SearchRequest::new("one", "/remote/only"),
            &files,
            &cancelled,
        )
    };
    assert!(matches!(search(), Err(Error::Cancelled)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    cancelled.store(false, Ordering::SeqCst);
    assert!(matches!(search(), Err(Error::Cancelled)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
}

#[test]
fn invalid_snapshot_paths_and_duplicate_files_fail_before_embedding() {
    let cache = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = SearchEngine::with_embedder(
        SembleConfig::new(cache.path()),
        Arc::new(ProbeEmbedder {
            calls: calls.clone(),
            cancel: None,
        }),
    );
    let running = AtomicBool::new(false);
    for files in [
        vec![file("../private.rs", "fn private() {}")],
        vec![file("/private.rs", "fn private() {}")],
        vec![file("x.rs", "fn a() {}"), file("x.rs", "fn b() {}")],
    ] {
        assert!(engine
            .search_snapshot(
                SearchRequest::new("private", "/remote/only"),
                &files,
                &running
            )
            .is_err());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
