// Copy to server/examples/ before compiling. Starts only an isolated listener and
// proxy; configuring and restoring Cursor settings belongs to the test operator.
use cursor_server::{api, config, cursor, store, Error, Result};
use cursor_server::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        TransportRegistry,
    },
    local_app::CaState,
    network::NetworkClients,
    plugin::{PluginRegistry, PluginRuntime},
    provider::ProviderRouter,
    search::WebCache,
    store::Store,
};
use std::{sync::Arc, time::Duration};

#[allow(dead_code)]
#[path = "../src/local_app/ca/mod.rs"]
mod ca;
#[allow(dead_code)]
#[path = "../src/local_app/proxy.rs"]
mod proxy;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).unwrap();
    if root == "decode" {
        return decode_trace().await;
    }
    let source_revision = std::env::args().nth(2).ok_or("expected source revision")?;
    if source_revision.len() != 40 || !source_revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("expected a full source commit SHA".into());
    }
    tracing_subscriber::fmt()
        .with_env_filter("cursor_server=info")
        .init();
    let root = std::path::PathBuf::from(root);
    let store = Store::connect(&format!("sqlite://{}", root.join("live.db").display())).await?;
    store.set_detailed_logging(true).await?;
    let clients = NetworkClients::new(store.clone());
    let plugins =
        PluginRegistry::managed(store.clone(), PluginRuntime::managed()?, "1.0.17".into())?;
    let provider = Arc::new(ProviderRouter::new(
        store.clone(),
        plugins,
        clients.clone(),
        Duration::from_secs(300),
        Duration::from_secs(120),
    ));
    let registry = TransportRegistry::with_web_cache(
        store.clone(),
        provider,
        PromptCompiler::new(PromptAssets::embedded()?),
        WebCache::at(root.join("web-cache"))?,
    );
    let router = cursor_server::api::router(registry.clone(), clients)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let backend = listener.local_addr()?;
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut proxy = proxy::ProxyRuntime::default();
    let tab_mode = Arc::new(parking_lot::RwLock::new(store.tab_settings().await?.mode));
    let (proxy_url, _) = proxy
        .start(backend, ca::CaManager::managed()?.load()?, 0, tab_mode)
        .await?;
    let session = serde_json::json!({"pid":std::process::id(),"backend":backend.to_string(),"proxy":proxy_url,"source_revision":source_revision});
    std::fs::write(
        root.join("live-session.json"),
        serde_json::to_vec_pretty(&session)?,
    )?;
    println!("READY {session}");
    tokio::signal::ctrl_c().await?;
    proxy.stop().await;
    registry.shutdown().await;
    task.abort();
    Ok(())
}

async fn decode_trace() -> std::result::Result<(), Box<dyn std::error::Error>> {
    use cursor_server::cursor::protocol::proto::{agent::v1 as pb, aiserver::v1 as ai};
    use prost::Message;
    use sqlx::Row;
    let root = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    let conversation = std::env::args().nth(3).unwrap();
    let db = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.join("live.db"))
            .read_only(true),
    )
    .await?;
    let rows = sqlx::query("SELECT a.request_id,a.seq,a.created_at_ms,b.data FROM cursor_run_trace_artifacts a JOIN cursor_run_traces t ON t.request_id=a.request_id JOIN blobs b ON b.blob_id=a.blob_id WHERE t.conversation_id=? AND a.artifact_type='bidi_request' AND json_extract(a.metadata_json,'$.message_type')='exec_client_message' ORDER BY a.created_at_ms,a.seq").bind(&conversation).fetch_all(&db).await?;
    let mut records = Vec::new();
    for row in rows {
        let append = ai::BidiAppendRequest::decode(row.get::<Vec<u8>, _>("data").as_slice())?;
        let message = pb::AgentClientMessage::decode(hex::decode(append.data)?.as_slice())?;
        let Some(pb::agent_client_message::Message::ExecClientMessage(exec)) = message.message
        else {
            continue;
        };
        let Some(pb::exec_client_message::Message::ShellStream(stream)) = exec.message else {
            continue;
        };
        use pb::shell_stream::Event;
        let event = match stream.event {
            Some(Event::Stdout(v)) => serde_json::json!({"type":"stdout","data":v.data}),
            Some(Event::Stderr(v)) => serde_json::json!({"type":"stderr","data":v.data}),
            Some(Event::Exit(v)) => {
                serde_json::json!({"type":"exit","code":v.code,"aborted":v.aborted,"local_execution_time_ms":v.local_execution_time_ms})
            }
            Some(Event::Start(v)) => {
                serde_json::json!({"type":"start","policy":v.sandbox_policy.as_ref().map(|p|p.r#type),"network":v.sandbox_policy.and_then(|p|p.network_access)})
            }
            Some(Event::Rejected(v)) => serde_json::json!({"type":"rejected","reason":v.reason}),
            Some(Event::PermissionDenied(_)) => serde_json::json!({"type":"permission_denied"}),
            Some(Event::SandboxUnsupported(v)) => {
                serde_json::json!({"type":"sandbox_unsupported","reason":v.reason})
            }
            Some(Event::Backgrounded(_)) => serde_json::json!({"type":"backgrounded"}),
            Some(Event::HookContext(_)) => serde_json::json!({"type":"hook_context"}),
            None => serde_json::json!({"type":"empty"}),
        };
        records.push(serde_json::json!({"request_id":row.get::<String,_>("request_id"),"trace_seq":row.get::<i64,_>("seq"),"created_at_ms":row.get::<i64,_>("created_at_ms"),"exec_id":exec.id,"event":event}));
    }
    let path = root.join("exec-evidence.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&records)?)?;
    println!("{} events -> {}", records.len(), path.display());
    Ok(())
}
