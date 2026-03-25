use chrono::{Duration, Utc};
use rtb_core::session::store::SessionStore;
use rtb_core::session::types::*;
use serde_json::json;
use std::io::Write;

fn make_meta(id: &str) -> SessionMeta {
    let now = Utc::now();
    SessionMeta {
        id: id.to_string(),
        name: format!("session-{}", id),
        session_type: SessionType::Terminal,
        agent: None,
        shell: Some("/bin/zsh".to_string()),
        cwd: "/tmp".to_string(),
        created_at: now,
        last_active: now,
        last_seq: 0,
        status: SessionStatus::Running,
        parent_id: None,
        tags: vec![],
    }
}

fn make_event(seq: u64) -> SessionEvent {
    SessionEvent {
        seq,
        event_type: "pty_out".to_string(),
        ts: Utc::now().timestamp_millis(),
        data: json!({"output": format!("line {}", seq)}),
    }
}

#[test]
fn test_create_and_get_meta() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    let mut meta = make_meta("abc123456789");
    meta.session_type = SessionType::Agent;
    meta.agent = Some(AgentInfo {
        provider: "claude-code".to_string(),
        model: "opus".to_string(),
    });
    meta.shell = None;
    meta.parent_id = Some("parent123".to_string());
    meta.tags = vec!["test".to_string(), "ci".to_string()];
    meta.last_seq = 42;

    store.create(&meta).expect("create session");

    let loaded = store.get_meta("abc123456789").expect("get meta");
    assert_eq!(loaded.id, "abc123456789");
    assert_eq!(loaded.name, "session-abc123456789");
    assert!(matches!(loaded.session_type, SessionType::Agent));
    assert!(loaded.agent.is_some());
    let agent = loaded.agent.unwrap();
    assert_eq!(agent.provider, "claude-code");
    assert_eq!(agent.model, "opus");
    assert!(loaded.shell.is_none());
    assert_eq!(loaded.cwd, "/tmp");
    assert_eq!(loaded.last_seq, 42);
    assert!(matches!(loaded.status, SessionStatus::Running));
    assert_eq!(loaded.parent_id, Some("parent123".to_string()));
    assert_eq!(loaded.tags, vec!["test", "ci"]);
}

#[test]
fn test_list_sessions() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    store.create(&make_meta("sess_aaa")).expect("create 1");
    store.create(&make_meta("sess_bbb")).expect("create 2");
    store.create(&make_meta("sess_ccc")).expect("create 3");

    let list = store.list().expect("list sessions");
    assert_eq!(list.len(), 3);

    let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"sess_aaa"));
    assert!(ids.contains(&"sess_bbb"));
    assert!(ids.contains(&"sess_ccc"));
}

#[test]
fn test_delete_session() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    store.create(&make_meta("to_delete")).expect("create");
    assert!(store.get_meta("to_delete").is_ok());

    store.delete("to_delete").expect("delete");
    assert!(store.get_meta("to_delete").is_err());

    let list = store.list().expect("list after delete");
    assert_eq!(list.len(), 0);
}

#[test]
fn test_append_and_read_events() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    store.create(&make_meta("evtsess")).expect("create");

    for i in 1..=5 {
        store
            .append_event("evtsess", &make_event(i))
            .expect("append");
    }

    let events = store.read_all_events("evtsess").expect("read all");
    assert_eq!(events.len(), 5);
    for (i, evt) in events.iter().enumerate() {
        assert_eq!(evt.seq, (i + 1) as u64);
        assert_eq!(evt.event_type, "pty_out");
    }
}

#[test]
fn test_read_events_since_seq() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    store.create(&make_meta("seqsess")).expect("create");

    for i in 1..=10 {
        store
            .append_event("seqsess", &make_event(i))
            .expect("append");
    }

    let events = store.read_events_since("seqsess", 5).expect("read since");
    assert_eq!(events.len(), 5);
    for (i, evt) in events.iter().enumerate() {
        assert_eq!(evt.seq, (i + 6) as u64);
    }
}

#[test]
fn test_corrupted_last_line() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    store.create(&make_meta("corruptsess")).expect("create");

    // Append 3 valid events
    for i in 1..=3 {
        store
            .append_event("corruptsess", &make_event(i))
            .expect("append");
    }

    // Manually append a corrupt line to events.jsonl
    let events_path = dir
        .path()
        .join("sessions")
        .join("corruptsess")
        .join("events.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&events_path)
        .expect("open events file");
    writeln!(file, "{{this is not valid json").expect("write corrupt line");

    let events = store.read_all_events("corruptsess").expect("read with corrupt");
    assert_eq!(events.len(), 3, "should skip corrupt line and return 3 valid events");
}

#[test]
fn test_cleanup_by_age() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    // Create an "old" session (40 days ago)
    let mut old_meta = make_meta("old_sess");
    old_meta.last_active = Utc::now() - Duration::days(40);
    old_meta.created_at = Utc::now() - Duration::days(40);
    store.create(&old_meta).expect("create old");

    // Create a "new" session (1 day ago)
    let mut new_meta = make_meta("new_sess");
    new_meta.last_active = Utc::now() - Duration::days(1);
    store.create(&new_meta).expect("create new");

    // Cleanup with max_age_days=30
    let deleted = store.cleanup(30, 10_000).expect("cleanup");
    assert_eq!(deleted, 1, "should delete 1 old session");

    let list = store.list().expect("list after cleanup");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "new_sess");
}

#[test]
fn test_update_meta() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = SessionStore::new(dir.path().join("sessions")).expect("create store");

    let meta = make_meta("upd_sess");
    store.create(&meta).expect("create");

    let mut updated = store.get_meta("upd_sess").expect("get meta");
    updated.status = SessionStatus::Exited;
    updated.last_seq = 100;
    updated.tags = vec!["updated".to_string()];

    store
        .update_meta("upd_sess", &updated)
        .expect("update meta");

    let loaded = store.get_meta("upd_sess").expect("get updated meta");
    assert!(matches!(loaded.status, SessionStatus::Exited));
    assert_eq!(loaded.last_seq, 100);
    assert_eq!(loaded.tags, vec!["updated"]);
}
