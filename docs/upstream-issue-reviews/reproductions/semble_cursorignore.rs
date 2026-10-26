//! Run as a temporary semble-core example; see the accompanying fix report.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use semble_core::{
    Embedder, FindRelatedRequest, SearchEngine, SearchRequest, SembleConfig, StaticEmbedder,
};
use serde_json::json;

struct ObservedEmbedder {
    inner: StaticEmbedder,
    private_texts: AtomicUsize,
}

impl Embedder for ObservedEmbedder {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }
    fn encode(&self, texts: &[String]) -> semble_core::Result<Vec<Vec<f32>>> {
        self.private_texts.fetch_add(
            texts
                .iter()
                .filter(|text| text.contains("SYNTHETIC_EXCLUDED_MARKER"))
                .count(),
            Ordering::SeqCst,
        );
        self.inner.encode(texts)
    }
}

fn includes_private(engine: &SearchEngine, root: &Path) -> bool {
    engine
        .search(SearchRequest::new("private_fixture", root))
        .unwrap()
        .results
        .iter()
        .any(|result| result.file_path == "private.py")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(args.len(), 2, "usage: RUN_DIRECTORY MODEL_ASSET_DIRECTORY");
    let run = PathBuf::from(&args[0]);
    let assets = PathBuf::from(&args[1]);
    fs::create_dir(&run)?; // Refuse to reuse a directory or modify an existing source tree.
    let root = run.join("source");
    fs::create_dir(&root)?;
    for directory in 0..20 {
        fs::create_dir(root.join(format!("src{directory}")))?;
        for file in 0..50 {
            fs::write(
                root.join(format!("src{directory}/fixture{file}.py")),
                format!("def public_{directory}_{file}(value):\n    return value + {file}\n"),
            )?;
        }
    }
    fs::write(
        root.join("private.py"),
        "def private_fixture():\n    return 'SYNTHETIC_EXCLUDED_MARKER'\n",
    )?;
    let embedder = Arc::new(ObservedEmbedder {
        inner: StaticEmbedder::load(
            &assets.join("model.safetensors"),
            &assets.join("tokenizer.json"),
        )?,
        private_texts: AtomicUsize::new(0),
    });
    let config = SembleConfig::new(run.join("cache"));
    let engine = SearchEngine::with_embedder(config.clone(), embedder.clone());
    let initial = engine.prepare(&root, &[])?;
    assert_eq!(initial.file_count, 1001);
    assert!(includes_private(&engine, &root));
    fs::write(root.join(".cursorignore"), "private.py\n")?;
    let started = Instant::now();
    let warm_excluded = !includes_private(&engine, &root);
    let rule_refresh_ms = started.elapsed().as_secs_f64() * 1000.0;
    let related_excluded = engine
        .find_related(FindRelatedRequest {
            repo: root.clone(),
            file_path: "private.py".into(),
            line: 1,
            top_k: 5,
            max_snippet_lines: Some(10),
            content: vec![],
        })
        .is_err();
    let disk = SearchEngine::with_embedder(config, embedder.clone());
    let disk_excluded = !includes_private(&disk, &root);
    let final_index = disk.prepare(&root, &[])?;
    let mut samples = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        engine.search(SearchRequest::new("calculate public value", &root))?;
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    embedder.private_texts.store(0, Ordering::SeqCst);
    let cold =
        SearchEngine::with_embedder(SembleConfig::new(run.join("cold-cache")), embedder.clone());
    let cold_excluded = !includes_private(&cold, &root);
    let excluded_body_encodings = embedder.private_texts.load(Ordering::SeqCst);
    let evidence = json!({
        "fixture": {"initial_files": initial.file_count, "public_files": 1000, "directories": 20},
        "model": embedder.id(), "dimensions": embedder.dimensions(),
        "warm_excluded": warm_excluded, "related_excluded": related_excluded,
        "disk_excluded": disk_excluded, "cold_excluded": cold_excluded,
        "excluded_body_encodings_in_cold_index": excluded_body_encodings,
        "final_index_files": final_index.file_count,
        "rule_refresh_ms": rule_refresh_ms,
        "warm_search_ms": {"samples": samples, "median": (samples[9]+samples[10])/2.0, "p95": samples[18]},
        "scope": "real StaticEmbedder on generated local files; not Cursor or remote Semble acceptance"
    });
    fs::write(
        run.join("result.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}
