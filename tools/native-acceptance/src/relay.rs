//! Test-only authenticated local relay. No token logging, network or product API.
use acp_ipc::{
    Location, read_frame,
    transport::{Listener, connect},
    write_frame,
};
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
            .starts_with("acp-s5-")
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
                let value = read_frame(&mut client).await?;
                let drop = value["method"] == "profile.mutate" && marker.exists();
                write_frame(&mut upstream, &value).await?;
                let reply = read_frame(&mut upstream).await?;
                if drop && reply.get("result").is_some() {
                    std::fs::write(
                        marker.with_file_name("dropped-receipt.json"),
                        serde_json::to_vec(&reply["result"]).unwrap(),
                    )
                    .unwrap();
                    std::fs::remove_file(&marker).unwrap();
                    return Ok::<(), acp_ipc::Error>(());
                }
                let dropped = marker.with_file_name("dropped-receipt.json");
                if value["method"] == "profile.mutate" && dropped.exists() {
                    let receipt: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(dropped).unwrap()).unwrap();
                    if reply["result"]["operation_id"] == receipt["operation_id"] {
                        assert_eq!(reply["result"], receipt, "immutable replay receipt differs");
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
