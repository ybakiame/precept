//! Test-only authenticated local relay. No token logging, network or product API.
use precept_ipc::{
    Location, read_frame,
    transport::{Listener, connect},
    write_frame,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

fn request_hash(value: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value["params"]).unwrap())
    )
}
pub async fn run(args: &[String]) {
    assert_eq!(args.len(), 4);
    let location = Location {
        runtime_dir: args[0].clone().into(),
        endpoint: args[1].clone(),
    };
    let alias = Location {
        runtime_dir: location.runtime_dir.clone(),
        endpoint: args[2].clone(),
    };
    let marker = std::path::PathBuf::from(&args[3]);
    assert_eq!(location.runtime_dir.file_name().unwrap(), "runtime");
    assert!(
        location
            .runtime_dir
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("precept-s5-")
    );
    assert_eq!(marker.parent(), location.runtime_dir.parent());
    let mut listener = Listener::bind(&alias).await.unwrap();
    println!("RELAY_READY");
    loop {
        let mut client = listener.accept().await.unwrap();
        let location = location.clone();
        let marker = marker.clone();
        tokio::spawn(async move {
            let mut upstream = connect(&location).await?;
            loop {
                let mut value = read_frame(&mut client).await?;
                let original_hash = request_hash(&value);
                let fault_path = marker.with_file_name("fault-next.json");
                let fault = if fault_path.exists() {
                    let candidate: Value =
                        serde_json::from_slice(&std::fs::read(&fault_path).unwrap()).unwrap();
                    if candidate["method"] == value["method"] {
                        std::fs::remove_file(&fault_path).unwrap();
                        candidate
                    } else {
                        Value::Null
                    }
                } else {
                    Value::Null
                };
                match fault["action"].as_str() {
                    Some("generation") => {
                        value["params"]["binding"]["generation"] = json!("0".repeat(32))
                    }
                    Some("profile") => {
                        value["params"]["binding"]["profile_id"] = json!("0".repeat(32))
                    }
                    Some("digest") => {
                        value["params"]["command"]["proposal_digest"] = json!("0".repeat(64))
                    }
                    Some("changed-bytes") => value["params"]["command"]["channel"] = json!("cli"),
                    Some("delay") | None => {}
                    _ => panic!("unsupported contained relay fault"),
                }
                let drop = value["method"] == "profile.mutate" && marker.exists();
                if value["method"] == "profile.mutate" {
                    use std::io::Write;
                    let count = marker.with_file_name("mutation-count");
                    let n = std::fs::read_to_string(&count)
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    assert!(n < 256, "bounded relay mutation count");
                    std::fs::write(count, (n + 1).to_string()).unwrap();
                    let mut log = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(marker.with_file_name("request-hashes.jsonl"))
                        .unwrap();
                    writeln!(log, "{}", json!({"operation_id":value["params"]["operation_id"],"request_sha256":request_hash(&value)})).unwrap();
                }
                write_frame(&mut upstream, &value).await?;
                let reply = read_frame(&mut upstream).await?;
                if value["method"] == "profile.mutate" && !reply["result"].is_null() {
                    std::fs::write(
                        marker.with_file_name("last-receipt.json"),
                        serde_json::to_vec(&reply["result"]).unwrap(),
                    )
                    .unwrap();
                }
                if fault["action"] == "delay" {
                    std::fs::write(marker.with_file_name("read-held"), b"held").unwrap();
                    let end = Instant::now() + Duration::from_millis(1500);
                    while !marker.with_file_name("release-read").exists() && Instant::now() < end {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                if !fault.is_null() {
                    std::fs::write(marker.with_file_name("fault-proof.json"), serde_json::to_vec(&json!({
                        "action":fault["action"],"method":value["method"],"original_request_sha256":original_hash,
                        "forwarded_request_sha256":request_hash(&value),"error":reply["error"],
                        "released_after_navigation":fault["action"] == "delay" && marker.with_file_name("release-read").exists(),
                        "real_upstream_response":true
                    })).unwrap()).unwrap();
                }
                if drop && !reply["result"].is_null() {
                    std::fs::write(
                        marker.with_file_name("dropped-receipt.json"),
                        serde_json::to_vec(&reply["result"]).unwrap(),
                    )
                    .unwrap();
                    std::fs::remove_file(&marker).unwrap();
                    std::fs::write(
                        marker.with_file_name("dropped-request.sha256"),
                        request_hash(&value),
                    )
                    .unwrap();
                    return Ok::<(), precept_ipc::Error>(());
                }
                let dropped = marker.with_file_name("dropped-receipt.json");
                if value["method"] == "profile.mutate" && dropped.exists() {
                    let receipt: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(dropped).unwrap()).unwrap();
                    if reply["result"]["operation_id"] == receipt["operation_id"] {
                        assert_eq!(reply["result"], receipt, "immutable replay receipt differs");
                        assert_eq!(
                            request_hash(&value),
                            std::fs::read_to_string(
                                marker.with_file_name("dropped-request.sha256")
                            )
                            .unwrap(),
                            "manual replay request bytes differ"
                        );
                        std::fs::write(
                            marker.with_file_name("replayed-receipt.json"),
                            serde_json::to_vec(&receipt).unwrap(),
                        )
                        .unwrap();
                    }
                }
                write_frame(&mut client, &reply).await?;
            }
        });
    }
}
