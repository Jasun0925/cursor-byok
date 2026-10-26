//! Diagnostic probe for PR #498; see the linked review for setup and scope.
use cursor_server::{
    cursor::{
        protocol::proto::agent::v1 as pb,
        tools::{
            runtime::{CursorToolRuntime, ExecContext},
            ToolBatchState, ToolDispatcher,
        },
    },
    model::ToolCall,
};
use semble_core::{
    index::IndexRepository, source::discover_files, ContentType, SearchEngine, SearchRequest,
    SembleConfig, StaticEmbedder,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Instant,
};

static ROOT: OnceLock<PathBuf> = OnceLock::new();
mod config {
    pub fn managed_data_dir() -> std::io::Result<std::path::PathBuf> {
        Ok(super::ROOT.get().unwrap().join("managed"))
    }
}
#[allow(dead_code)]
#[path = "issue469_snapshot_source/mod.rs"]
mod host_snapshot;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args().nth(1).unwrap());
    let assets = PathBuf::from(
        std::env::args()
            .nth(3)
            .expect("model asset directory required"),
    );
    prepare_fixture(&root, &assets)?;
    ROOT.set(root.clone()).unwrap();
    let remote = std::env::args().nth(2).unwrap();
    assert!(!PathBuf::from(&remote).exists());
    let runtime = CursorToolRuntime::default();
    let dispatcher = ToolDispatcher::new(runtime.clone());
    let arguments = json!({"repo":remote,"query":"remote_semble_probe"});
    let call = ToolCall {
        index: 0,
        call_id: "audit-search".into(),
        model_call_id: "audit-model".into(),
        name: "SembleSearch".into(),
        arguments_text: arguments.to_string(),
        arguments,
        argument_error: None,
    };
    let dispatched = dispatcher
        .start_batch(
            &[call],
            ToolBatchState {
                completed: &HashSet::new(),
                started: &HashSet::new(),
                response_text: "",
                response_thinking: "",
            },
            &[],
            &BTreeMap::new(),
            &ExecContext {
                conversation_id: "audit-only".into(),
                ..Default::default()
            },
        )
        .await?;
    let exec_count = dispatched
        .iter()
        .flat_map(|item| &item.messages)
        .filter(|message| {
            matches!(
                &message.message,
                Some(pb::agent_server_message::Message::ExecServerMessage(_))
            )
        })
        .count();
    assert_eq!(exec_count, 0);
    assert!(runtime.running_exec_ids().await.is_empty());
    let model = root.join("cache/models/potion-code-16M-v2");
    let embedder = Arc::new(StaticEmbedder::load(
        &model.join("model.safetensors"),
        &model.join("tokenizer.json"),
    )?);
    let engine =
        SearchEngine::with_embedder(SembleConfig::new(root.join("cache")), embedder.clone());
    let remote_error = engine
        .search(SearchRequest::new("remote_semble_probe", &remote))
        .unwrap_err()
        .to_string();
    let control = engine.search(SearchRequest::new(
        "remote_semble_probe",
        root.join("local-control"),
    ))?;
    assert!(control.results.iter().any(|item| item
        .content
        .as_ref()
        .is_some_and(|text| text.contains("REMOTE_SEMBLE_FIXTURE_20261027"))));
    let source = root.join("source");
    let before = discover_files(&source, &[ContentType::Code], 1_000_000)?
        .into_iter()
        .map(|f| f.relative_path)
        .collect::<Vec<_>>();
    assert!(!before.contains(&"ignored.py".to_string()));
    assert!(!before.contains(&"semble-private.py".to_string()));
    let command = host_snapshot::snapshot_shell_command(source.to_str().unwrap());
    let original = std::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .output()?;
    assert_eq!(original.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&original.stdout).contains("not a directory"));
    // Audit the later stages only after recording the unmodified command failure.
    let corrected = command.replace(
        "exec(base64.b64decode(sys.argv[1]))",
        "code=base64.b64decode(sys.argv.pop(1));exec(code)",
    );
    assert_ne!(command, corrected);
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(corrected)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8(output.stdout)?;
    let dump = host_snapshot::parse_dump(&stdout)?;
    let transmitted = dump
        .files
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let materialized =
        host_snapshot::materialize_dump("audit", "call-1", "/workspace/project", &dump)?;
    let after = discover_files(&materialized, &[ContentType::Code], 1_000_000)?
        .into_iter()
        .map(|f| f.relative_path)
        .collect::<Vec<_>>();
    assert!(after.contains(&"ignored.py".to_string()));
    assert!(!after.contains(&"semble-private.py".to_string()));
    let ignored_result = engine.search(SearchRequest::new("gitignored_probe", &materialized))?;
    assert!(ignored_result
        .results
        .iter()
        .any(|r| r.file_path == "ignored.py"
            && r.content
                .as_ref()
                .is_some_and(|s| s.contains("GITIGNORE_TEST_ONLY"))));

    let repository = IndexRepository::new(SembleConfig::new(root.join("identity-cache")), embedder);
    let identity = host_snapshot::source_identity("/workspace/project");
    let a = repository.load_for_search(&root.join("host-a"), &identity, &[ContentType::Code])?;
    let started = Instant::now();
    let b = repository.load_for_search(&root.join("host-b"), &identity, &[ContentType::Code])?;
    let second_call_ms = started.elapsed().as_millis();
    assert!(second_call_ms < 1000);
    assert!(Arc::ptr_eq(&a, &b));
    assert!(b.metadata.files.iter().any(|f| f.path == "alpha.py"));
    let refreshed =
        repository.load_or_build(&root.join("host-b"), &identity, &[ContentType::Code])?;
    assert!(refreshed.metadata.files.iter().any(|f| f.path == "beta.py"));
    let report = json!({"base":"3d309480d55ece3021688dd36228ad48b76cd56c","candidate":"c7114878c33076340c0c2f1e77ce4e7512d1960e","baseline":{"remote_exists_on_byok_host":false,"cursor_exec_messages":exec_count,"error":remote_error,"local_control":control},"unmodified_snapshot_command":{"exit_code":original.status.code(),"stdout":String::from_utf8_lossy(&original.stdout)},"snapshot_after_argument_correction_only":{"serialized":transmitted,"original_discovery":before,"materialized_discovery":after,"ignored_search_result":ignored_result},"identity":{"source_identity":identity,"second_call_ms":second_call_ms,"second_reused_first_index":Arc::ptr_eq(&a,&b),"host_a_files":a.metadata.files.iter().map(|f|&f.path).collect::<Vec<_>>(),"host_b_cached_files":b.metadata.files.iter().map(|f|&f.path).collect::<Vec<_>>(),"host_b_refreshed_files":refreshed.metadata.files.iter().map(|f|&f.path).collect::<Vec<_>>()},"scope":"real core embedder and native dispatcher; synthetic files; no live Cursor UI or provider; candidate source extraction plus current core APIs; transfer and index findings require correcting the separate launcher argument defect"});
    std::fs::write(
        root.join("audit-result.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn prepare_fixture(root: &std::path::Path, assets: &std::path::Path) -> std::io::Result<()> {
    // Require a new audit directory: never replace a user's existing files.
    std::fs::create_dir(root)?;
    let model = root.join("cache/models/potion-code-16M-v2");
    std::fs::create_dir_all(&model)?;
    for name in ["model.safetensors", "tokenizer.json"] {
        std::fs::copy(assets.join(name), model.join(name))?;
    }
    for dir in ["local-control", "source", "host-a", "host-b"] {
        std::fs::create_dir(root.join(dir))?;
    }
    for (name, text) in [
        (
            "local-control/auth.py",
            "def remote_semble_probe():\n    return \"REMOTE_SEMBLE_FIXTURE_20261027\"\n",
        ),
        (
            "source/visible.py",
            "def visible_probe():\n    return \"VISIBLE_TEST_ONLY\"\n",
        ),
        (
            "source/ignored.py",
            "def gitignored_probe():\n    return \"GITIGNORE_TEST_ONLY\"\n",
        ),
        (
            "source/private.py",
            "def cursorignored_probe():\n    return \"CURSORIGNORE_TEST_ONLY\"\n",
        ),
        (
            "source/semble-private.py",
            "def sembleignored_probe():\n    return \"SEMBLEIGNORE_TEST_ONLY\"\n",
        ),
        ("source/.gitignore", "ignored.py\n"),
        ("source/.cursorignore", "private.py\n"),
        ("source/.sembleignore", "semble-private.py\n"),
        (
            "host-a/alpha.py",
            "def alpha_only():\n    return \"HOST_A_TEST_ONLY\"\n",
        ),
        (
            "host-b/beta.py",
            "def beta_only():\n    return \"HOST_B_TEST_ONLY\"\n",
        ),
    ] {
        std::fs::write(root.join(name), text)?;
    }
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(root.join("source"))
        .status()?;
    assert!(status.success());
    Ok(())
}
