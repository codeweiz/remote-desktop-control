use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;

use rtb_core::pty::buffer::RingBuffer;
use rtb_core::pty::manager::PtyManager;

// ---------------------------------------------------------------------------
// RingBuffer tests
// ---------------------------------------------------------------------------

#[test]
fn test_ring_buffer_push_and_read() {
    let buf = RingBuffer::new(10);

    buf.push(1, Bytes::from("hello"));
    buf.push(2, Bytes::from("world"));

    assert_eq!(buf.len(), 2);
    assert_eq!(buf.last_seq(), 2);

    let items = buf.get_last_n(10);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].0, 1);
    assert_eq!(items[0].1, Bytes::from("hello"));
    assert_eq!(items[1].0, 2);
    assert_eq!(items[1].1, Bytes::from("world"));
}

#[test]
fn test_ring_buffer_capacity() {
    let buf = RingBuffer::new(3);

    buf.push(1, Bytes::from("a"));
    buf.push(2, Bytes::from("b"));
    buf.push(3, Bytes::from("c"));
    buf.push(4, Bytes::from("d"));
    buf.push(5, Bytes::from("e"));

    // Capacity is 3, so oldest items (seq 1, 2) should be evicted
    assert_eq!(buf.len(), 3);
    assert_eq!(buf.last_seq(), 5);

    let items = buf.get_last_n(10);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].0, 3);
    assert_eq!(items[1].0, 4);
    assert_eq!(items[2].0, 5);
}

#[test]
fn test_ring_buffer_get_since() {
    let buf = RingBuffer::new(10);

    buf.push(1, Bytes::from("a"));
    buf.push(2, Bytes::from("b"));
    buf.push(3, Bytes::from("c"));
    buf.push(4, Bytes::from("d"));
    buf.push(5, Bytes::from("e"));

    // Get events with seq > 3
    let items = buf.get_since(3);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].0, 4);
    assert_eq!(items[0].1, Bytes::from("d"));
    assert_eq!(items[1].0, 5);
    assert_eq!(items[1].1, Bytes::from("e"));

    // Get events with seq > 0 (all events)
    let items = buf.get_since(0);
    assert_eq!(items.len(), 5);

    // Get events with seq > 5 (none)
    let items = buf.get_since(5);
    assert_eq!(items.len(), 0);

    // Get events with seq > 100 (none)
    let items = buf.get_since(100);
    assert_eq!(items.len(), 0);
}

#[test]
fn test_ring_buffer_get_last_n() {
    let buf = RingBuffer::new(10);

    buf.push(1, Bytes::from("a"));
    buf.push(2, Bytes::from("b"));
    buf.push(3, Bytes::from("c"));

    let items = buf.get_last_n(2);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].0, 2);
    assert_eq!(items[1].0, 3);

    // Request more than available
    let items = buf.get_last_n(100);
    assert_eq!(items.len(), 3);
}

#[test]
fn test_ring_buffer_empty() {
    let buf = RingBuffer::new(10);
    assert_eq!(buf.len(), 0);
    assert_eq!(buf.last_seq(), 0);
    assert!(buf.get_since(0).is_empty());
    assert!(buf.get_last_n(5).is_empty());
}

// ---------------------------------------------------------------------------
// PTY session tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_pty_session() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus, config);

    let session_id = manager
        .create_session("test-session", Some("/bin/sh"), None)
        .await
        .expect("should create session");

    assert!(!session_id.is_empty());

    let session = manager.get_session(&session_id);
    assert!(session.is_some(), "session should exist after creation");

    let session = session.unwrap();
    assert_eq!(session.name(), "test-session");
    assert!(session.is_running());

    // Cleanup
    manager.kill_session(&session_id).await.expect("should kill session");
}

#[tokio::test]
async fn test_pty_output() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus.clone(), config);

    let session_id = manager
        .create_session("echo-test", Some("/bin/sh"), None)
        .await
        .expect("should create session");

    // Subscribe to data events for this session
    let mut rx = event_bus.create_data_subscriber(&session_id);

    // Give the shell a moment to start up, then send the echo command.
    // Retry the write a few times because the shell may need extra time
    // in resource-constrained environments (CI, heavy load, etc.).
    tokio::time::sleep(Duration::from_millis(500)).await;

    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        manager
            .write_input(&session_id, b"echo hello\n")
            .expect("should write input");

        // Drain any events for a short window to see if "hello" appears
        let check_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = check_deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(rtb_core::events::DataEvent::PtyOutput { data, .. })) => {
                    let text = String::from_utf8_lossy(&data);
                    if text.contains("hello") {
                        // Success — verify ring buffer also captured output
                        let session = manager.get_session(&session_id).unwrap();
                        let buffer_items = session.buffer().get_last_n(100);
                        assert!(!buffer_items.is_empty(), "ring buffer should have captured output");

                        // Cleanup
                        manager.kill_session(&session_id).await.expect("should kill session");
                        return; // PASS
                    }
                }
                Ok(Some(_)) => { /* other event type */ }
                Ok(None) => break, // channel closed
                Err(_) => break,   // timeout
            }
        }
    }

    // Cleanup before failing
    let _ = manager.kill_session(&session_id).await;
    panic!("should have received output containing 'hello' after 3 attempts");
}

#[tokio::test]
async fn test_pty_manager_crud() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus, config);

    // Create two sessions
    let id1 = manager
        .create_session("session-1", Some("/bin/sh"), None)
        .await
        .expect("should create session 1");
    let id2 = manager
        .create_session("session-2", Some("/bin/sh"), None)
        .await
        .expect("should create session 2");

    // List sessions — should have 2
    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 2, "should list 2 sessions");

    // Verify both sessions are present by name
    let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"session-1"));
    assert!(names.contains(&"session-2"));

    // Kill one session
    manager
        .kill_session(&id1)
        .await
        .expect("should kill session 1");

    // List sessions — should have 1
    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 1, "should list 1 session after kill");
    assert_eq!(sessions[0].name, "session-2");

    // Verify killed session is gone
    assert!(
        manager.get_session(&id1).is_none(),
        "killed session should not be retrievable"
    );

    // Kill remaining session
    manager
        .kill_session(&id2)
        .await
        .expect("should kill session 2");

    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 0, "should list 0 sessions after all killed");
}

#[tokio::test]
async fn test_pty_resize() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus, config);

    let session_id = manager
        .create_session("resize-test", Some("/bin/sh"), None)
        .await
        .expect("should create session");

    // Resize should succeed
    manager
        .resize(&session_id, 120, 40)
        .expect("should resize session");

    // Cleanup
    manager.kill_session(&session_id).await.expect("should kill session");
}

#[tokio::test]
async fn test_kill_nonexistent_session() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus, config);

    let result = manager.kill_session("nonexistent").await;
    assert!(result.is_err(), "killing nonexistent session should fail");
}

#[tokio::test]
async fn test_write_nonexistent_session() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus, config);

    let result = manager.write_input("nonexistent", b"hello");
    assert!(result.is_err(), "writing to nonexistent session should fail");
}
