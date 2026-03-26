use std::sync::Arc;
use std::time::Duration;

use rtb_core::pty::manager::PtyManager;

// ---------------------------------------------------------------------------
// PTY session tests (tmux-backed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_pty_session() {
    let event_bus = Arc::new(rtb_core::event_bus::EventBus::new());
    let config = Arc::new(rtb_core::config::Config::default());
    let manager = PtyManager::new(event_bus, config);

    let session_id = manager
        .create_session("test-session", None)
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
        .create_session("echo-test", None)
        .await
        .expect("should create session");

    // Subscribe to live output from the session's broadcast channel
    let session = manager.get_session(&session_id).expect("session should exist");
    let mut rx = session.subscribe();

    // Give the shell a moment to start up, then send the echo command.
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
                Ok(Ok(data)) => {
                    let text = String::from_utf8_lossy(&data);
                    if text.contains("hello") {
                        // Cleanup
                        manager.kill_session(&session_id).await.expect("should kill session");
                        return; // PASS
                    }
                }
                Ok(Err(_)) => break, // lagged or closed
                Err(_) => break,     // timeout
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
        .create_session("session-1", None)
        .await
        .expect("should create session 1");
    let id2 = manager
        .create_session("session-2", None)
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
        .create_session("resize-test", None)
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
