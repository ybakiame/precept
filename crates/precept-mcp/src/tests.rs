use crate::{
    binding::root_dirs,
    protocol::{info, memory_reply, suggestion_reply, tools},
};
use precept_ipc::Error;
use serde_json::json;

#[test]
fn server_metadata_uses_the_package_version() {
    let info = info();
    assert_eq!(info["name"], "precept");
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn schemas_expose_read_suggest_session_and_memory_tools_none_naming_a_project() {
    let v = tools();
    assert_eq!(v.as_array().unwrap().len(), 12);
    for t in v.as_array().unwrap() {
        assert_eq!(t["inputSchema"]["additionalProperties"], false);
        assert!(
            !t["inputSchema"]["properties"]
                .as_object()
                .unwrap()
                .contains_key("project")
        );
    }
}

#[test]
fn memory_replies_mark_notes_as_unreviewed_and_say_whether_to_retry() {
    let ok = memory_reply(
        "memory.recall",
        Ok(json!({"entries":[{"kind":"caution","title":"WSL","id":"ab","body":"slow"}]})),
    );
    let text = ok["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("unreviewed") && text.contains("- [caution] WSL (ab): slow"));
    let none = memory_reply("memory.recall", Ok(json!({"entries":[]})));
    assert!(
        none["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No memory")
    );
    // Index lines carry no body and say how to fetch the full text.
    let index = memory_reply(
        "memory.recall",
        Ok(json!({"index":true,"entries":[
            {"kind":"caution","title":"WSL","id":"ab","weight":61.0}
        ]})),
    );
    let text = index["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("- [caution] WSL (ab weight 61.0)") && text.contains("recall id="));
    assert!(!text.contains("slow"));
    // A fetch by id and a stale notice.
    let got = memory_reply(
        "memory.get",
        Ok(
            json!({"entry":{"kind":"lesson","title":"Run","id":"cd","body":"cargo test",
            "notice":"Not verified for 9 days; check it."}}),
        ),
    );
    let text = got["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("- [lesson] Run (cd): cargo test")
            && text.contains("Not verified for 9 days")
    );
    for (e, needle) in [
        (Error::Unauthorized, "Do not retry"),
        (Error::NotFound, "recall"),
        (Error::InvalidRequest, "at most 120"),
    ] {
        let r = memory_reply("memory.remember", Err(e));
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"].as_str().unwrap().contains(needle));
    }
}

#[test]
fn roots_name_local_folders_only() {
    let dirs = root_dirs(&json!({"roots":[
        {"uri":"file:///home/u/my%20repo","name":"repo"},
        {"uri":"https://example.invalid/x"}
    ]}));
    assert_eq!(dirs, vec![std::path::PathBuf::from("/home/u/my repo")]);
    assert!(root_dirs(&json!({})).is_empty());
    // A Windows drive URI keeps its drive letter, not a leading slash.
    let dirs = root_dirs(&json!({"roots":[
        {"uri":"file:///C:/Users/u/repo"},
        {"uri":"file:///c%3A/Users/u/other"}
    ]}));
    if cfg!(windows) {
        assert_eq!(
            dirs,
            vec![
                std::path::PathBuf::from("C:/Users/u/repo"),
                std::path::PathBuf::from("c:/Users/u/other")
            ]
        );
    } else {
        assert_eq!(dirs.len(), 2);
    }
}

#[test]
fn suggestion_replies_say_it_is_not_active_and_whether_to_retry() {
    let ok = suggestion_reply(Ok(json!({"status":"suggested","id":"7"})));
    assert_eq!(ok["isError"], false);
    let text = ok["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("#7") && text.contains("not active"));
    for (e, needle) in [
        (Error::Unauthorized, "not allowed"),
        (Error::Unavailable, "writes are off"),
        (Error::Conflict, "already"),
        (Error::Capacity, "limit"),
        (Error::InvalidRequest, "Invalid suggestion"),
    ] {
        let r = suggestion_reply(Err(e));
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"].as_str().unwrap().contains(needle));
    }
    assert!(
        tools()
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "suggest_rule")
    );
}
