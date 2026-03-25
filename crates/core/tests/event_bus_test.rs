use rtb_core::event_bus::EventBus;
use rtb_core::events::{AgentContent, ControlEvent, DataEvent, SessionType};
use bytes::Bytes;

#[tokio::test]
async fn test_broadcast_control_event() {
    let bus = EventBus::new();
    let mut rx = bus.subscribe_control();

    bus.publish_control(ControlEvent::SessionCreated {
        session_id: "s1".into(),
        session_type: SessionType::Terminal,
    });

    let event = rx.recv().await.expect("should receive control event");
    match event.as_ref() {
        ControlEvent::SessionCreated { session_id, session_type } => {
            assert_eq!(session_id, "s1");
            assert!(matches!(session_type, SessionType::Terminal));
        }
        _ => panic!("unexpected event variant"),
    }
}

#[tokio::test]
async fn test_multiple_control_subscribers() {
    let bus = EventBus::new();
    let mut rx1 = bus.subscribe_control();
    let mut rx2 = bus.subscribe_control();
    let mut rx3 = bus.subscribe_control();

    bus.publish_control(ControlEvent::TunnelReady {
        url: "https://example.com".into(),
    });

    for rx in [&mut rx1, &mut rx2, &mut rx3] {
        let event = rx.recv().await.expect("subscriber should receive event");
        match event.as_ref() {
            ControlEvent::TunnelReady { url } => {
                assert_eq!(url, "https://example.com");
            }
            _ => panic!("unexpected event variant"),
        }
    }
}

#[tokio::test]
async fn test_data_event_per_session() {
    let bus = EventBus::new();
    let mut rx = bus.create_data_subscriber("session-1");

    bus.publish_data(
        "session-1",
        DataEvent::PtyOutput {
            seq: 1,
            data: Bytes::from("hello"),
        },
    )
    .await;

    let event = rx.recv().await.expect("should receive data event");
    match event {
        DataEvent::PtyOutput { seq, data } => {
            assert_eq!(seq, 1);
            assert_eq!(data, Bytes::from("hello"));
        }
        _ => panic!("unexpected event variant"),
    }
}

#[tokio::test]
async fn test_data_event_isolation() {
    let bus = EventBus::new();
    let mut rx_a = bus.create_data_subscriber("session-a");
    let mut rx_b = bus.create_data_subscriber("session-b");

    // Publish to session-a only
    bus.publish_data(
        "session-a",
        DataEvent::PtyOutput {
            seq: 1,
            data: Bytes::from("for A"),
        },
    )
    .await;

    // session-a subscriber should receive it
    let event = rx_a.recv().await.expect("session-a should receive event");
    match event {
        DataEvent::PtyOutput { seq, data } => {
            assert_eq!(seq, 1);
            assert_eq!(data, Bytes::from("for A"));
        }
        _ => panic!("unexpected event variant"),
    }

    // session-b subscriber should NOT receive anything
    // Use try_recv to check without blocking
    let result = rx_b.try_recv();
    assert!(
        result.is_err(),
        "session-b should not receive events for session-a"
    );
}

#[tokio::test]
async fn test_remove_session_cleans_up() {
    let bus = EventBus::new();
    let mut rx = bus.create_data_subscriber("session-x");

    // Publish something first to confirm the channel works
    bus.publish_data(
        "session-x",
        DataEvent::PtyExited { exit_code: 0 },
    )
    .await;
    let _ = rx.recv().await.expect("should receive event before removal");

    // Remove the session
    bus.remove_session("session-x");

    // After removal, the receiver should be closed (returns None)
    let result = rx.recv().await;
    assert!(
        result.is_none(),
        "receiver should be closed after session removal"
    );

    // The session entry should no longer exist in the internal map
    assert!(!bus.has_session("session-x"));
}

#[tokio::test]
async fn test_dead_sender_cleanup() {
    let bus = EventBus::new();

    // Create two subscribers for the same session
    let rx1 = bus.create_data_subscriber("session-d");
    let mut rx2 = bus.create_data_subscriber("session-d");

    // Confirm both senders are registered
    assert_eq!(bus.subscriber_count("session-d"), 2);

    // Drop the first receiver, making its sender "dead"
    drop(rx1);

    // Publish an event — this should trigger cleanup of the dead sender
    bus.publish_data(
        "session-d",
        DataEvent::PtyOutput {
            seq: 42,
            data: Bytes::from("cleanup test"),
        },
    )
    .await;

    // The surviving subscriber should still receive the event
    let event = rx2.recv().await.expect("surviving subscriber should receive event");
    match event {
        DataEvent::PtyOutput { seq, data } => {
            assert_eq!(seq, 42);
            assert_eq!(data, Bytes::from("cleanup test"));
        }
        _ => panic!("unexpected event variant"),
    }

    // After publish, dead sender should have been cleaned up
    assert_eq!(bus.subscriber_count("session-d"), 1);
}

#[tokio::test]
async fn test_multiple_data_subscribers_same_session() {
    let bus = EventBus::new();
    let mut rx1 = bus.create_data_subscriber("session-m");
    let mut rx2 = bus.create_data_subscriber("session-m");

    bus.publish_data(
        "session-m",
        DataEvent::AgentMessage {
            seq: 10,
            content: AgentContent::Text {
                text: "hello".into(),
                streaming: false,
            },
        },
    )
    .await;

    // Both subscribers should receive the event
    for rx in [&mut rx1, &mut rx2] {
        let event = rx.recv().await.expect("subscriber should receive event");
        match event {
            DataEvent::AgentMessage { seq, .. } => {
                assert_eq!(seq, 10);
            }
            _ => panic!("unexpected event variant"),
        }
    }
}

#[tokio::test]
async fn test_publish_data_to_nonexistent_session() {
    let bus = EventBus::new();

    // Publishing to a session that has no subscribers should not panic
    bus.publish_data(
        "nonexistent",
        DataEvent::PtyOutput {
            seq: 1,
            data: Bytes::from("nowhere"),
        },
    )
    .await;
}
