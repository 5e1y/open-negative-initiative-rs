//! A transfer stopped halfway: by the sender's cancel, by its hanging up, by the receiver; nothing is left.

use super::*;

/// Starts a 10 MiB upload, sends 2 MiB of it and waits until the part file exists.
async fn half_upload(
    raw: &Raw,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    dst: &std::path::Path,
) -> (PrepareUploadResponse, tokio::task::JoinHandle<Option<u16>>, tokio::sync::mpsc::Sender<std::io::Result<Vec<u8>>>)
{
    let session = raw.prepare_ok(vec![meta("f", "big.dng", 10 << 20, None)]).await;
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<Vec<u8>>>(4);
    let body = futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|chunk| (chunk, rx)) });
    let request = raw.client.post(raw.upload_url(&session, "f")).body(reqwest::Body::wrap_stream(body));
    let task = tokio::spawn(async move { request.send().await.ok().map(|r| r.status().as_u16()) });
    tx.send(Ok(vec![1u8; 2 << 20])).await.unwrap();
    wait_for(events, |e| matches!(e, Event::FileProgress { received, .. } if *received >= 1 << 20).then_some(())).await;
    let parts = names(dst);
    assert!(parts.len() == 1 && parts[0].ends_with(".part"), "a part file is being written: {parts:?}");
    (session, task, tx)
}

#[tokio::test]
async fn cancel_mid_transfer_leaves_nothing() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, mut events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let (session, task, _tx) = half_upload(&raw, &mut events, dst.path()).await;

    assert_eq!(raw.cancel(&session.session_id).await, 200);
    wait_for(&mut events, |e| matches!(e, Event::SessionCancelled { .. }).then_some(())).await;
    let status = tokio::time::timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
    assert_ne!(status, Some(200));
    eventually(|| names(dst.path()).is_empty()).await;
    raw.prepare_ok(vec![meta("g", "next", 1, None)]).await;
}

#[tokio::test]
async fn a_sender_hanging_up_mid_transfer_leaves_nothing() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, mut events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let (_session, task, tx) = half_upload(&raw, &mut events, dst.path()).await;

    // A failing body makes the client drop the connection mid-chunk, as a sender that dies does.
    tx.send(Err(std::io::Error::other("sender gone"))).await.unwrap();
    assert_eq!(tokio::time::timeout(Duration::from_secs(5), task).await.unwrap().unwrap(), None);
    wait_for(&mut events, |e| matches!(e, Event::FileFailed { .. }).then_some(())).await;
    wait_for(&mut events, |e| matches!(e, Event::SessionFinished { .. }).then_some(())).await;
    eventually(|| names(dst.path()).is_empty()).await;
}

#[tokio::test]
async fn the_receiver_can_cancel_its_session() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, mut events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let (_session, task, _tx) = half_upload(&raw, &mut events, dst.path()).await;

    receiver.cancel_session();
    wait_for(&mut events, |e| matches!(e, Event::SessionCancelled { .. }).then_some(())).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), task).await.unwrap();
    eventually(|| names(dst.path()).is_empty()).await;
}
