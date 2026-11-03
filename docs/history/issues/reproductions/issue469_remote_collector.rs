// Copy into server/examples/ to compile against the current production modules.
// Arguments: remote workspace root, local JSONL file of prior stdout responses.
// Emit the next exact shell command, or verified file hashes after completion.
// Execute commands over SSH in the workspace; append each stdout as one JSONL line.
#[allow(dead_code, unused_imports)]
#[path = "../src/search/snapshot/mod.rs"]
mod snapshot;

use serde_json::json;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut collector =
        snapshot::Collector::new(args[1].clone(), vec![semble_core::ContentType::Code]);
    for response in std::fs::read_to_string(&args[2])?.lines() {
        if let Some(files) = collector.accept(response)? {
            let files: Vec<_> = files
                .iter()
                .map(|file| {
                    json!({
                        "path": file.relative_path,
                        "bytes": file.bytes.len(),
                        "sha256": hex::encode(Sha256::digest(&file.bytes)),
                    })
                })
                .collect();
            println!("{}", json!({"done":true,"files":files}));
            return Ok(());
        }
    }
    println!("{}", json!({"done":false,"command":collector.command()?}));
    Ok(())
}
