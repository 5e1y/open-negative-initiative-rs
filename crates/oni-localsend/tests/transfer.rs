mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use common::*;
use futures_util::future::BoxFuture;
use oni_localsend::*;

struct RejectAll;

impl AcceptPolicy for RejectAll {
    fn decide<'a>(&'a self, _: &'a DeviceInfo, _: &'a [FileMeta], _: Option<&'a str>) -> BoxFuture<'a, Decision> {
        Box::pin(async { Decision::Reject })
    }
}

/// Accepts only files whose name ends with `.dng`.
struct OnlyDng;

impl AcceptPolicy for OnlyDng {
    fn decide<'a>(&'a self, _: &'a DeviceInfo, files: &'a [FileMeta], _: Option<&'a str>) -> BoxFuture<'a, Decision> {
        let ids = files.iter().filter(|f| f.file_name.ends_with(".dng")).map(|f| f.id.clone()).collect();
        Box::pin(async move { Decision::Accept(ids) })
    }
}

/// Accepts a sender that presents the secret it was handed, whatever it claims to be.
struct OnlyPin;

impl AcceptPolicy for OnlyPin {
    fn decide<'a>(&'a self, _: &'a DeviceInfo, _: &'a [FileMeta], pin: Option<&'a str>) -> BoxFuture<'a, Decision> {
        Box::pin(async move { if pin == Some("handed-out") { Decision::AcceptAll } else { Decision::Reject } })
    }
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[tokio::test]
async fn round_trip_of_three_files_is_byte_exact() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut big = vec![0u8; 20 << 20];
    rand::fill(&mut big[..]);
    let files = [("hello.txt", b"hello, world\n".to_vec()), ("P1072392.RW2", big), ("empty.bin", Vec::new())];
    let paths: Vec<_> = files.iter().map(|(n, b)| write(src.path(), n, b)).collect();
    let (receiver, mut events) = start(dst.path(), |_| {}).await;

    let report = send_files(&me("S1"), &target(&receiver), &paths, None).await.unwrap();
    assert_eq!(report.sent.len(), 3);
    assert!(report.skipped.is_empty());

    let session = report.session_id.unwrap();
    let started = wait_for(&mut events, |e| match e {
        Event::SessionStarted { session_id, files, sender, .. } => {
            Some((session_id.clone(), files.len(), sender.alias.clone()))
        }
        _ => None,
    })
    .await;
    assert_eq!(started, (session.clone(), 3, "Sender".into()));
    let mut done = Vec::new();
    let finished = wait_for(&mut events, |e| match e {
        Event::FileDone { path, .. } => {
            done.push(path.clone());
            None
        }
        Event::SessionFinished { session_id } => Some(session_id.clone()),
        other => {
            assert!(matches!(other, Event::FileProgress { .. }), "unexpected {other:?}");
            None
        }
    })
    .await;
    assert_eq!(finished, session);
    assert_eq!(done.len(), 3);

    assert_eq!(names(dst.path()), ["P1072392.RW2", "empty.bin", "hello.txt"], "no part file left");
    for (name, bytes) in &files {
        assert!(std::fs::read(dst.path().join(name)).unwrap() == *bytes, "{name} differs");
    }
}

#[tokio::test]
async fn a_lying_checksum_is_refused_and_leaves_nothing() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, mut events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let bytes = vec![7u8; 3 << 20];

    let lie = raw.prepare_ok(vec![meta("f", "lie.dng", bytes.len() as u64, Some(sha256_hex(b"other")))]).await;
    assert_eq!(raw.upload(&lie, "f", bytes.clone()).await, 422);
    let reason = wait_for(&mut events, |e| match e {
        Event::FileFailed { reason, .. } => Some(reason.clone()),
        _ => None,
    })
    .await;
    assert!(reason.contains("sha256"));
    wait_for(&mut events, |e| matches!(e, Event::SessionFinished { .. }).then_some(())).await;
    assert!(names(dst.path()).is_empty(), "nothing written: {:?}", names(dst.path()));

    // The same harness with the true checksum succeeds, so the 422 above is the checksum's doing.
    let truth =
        raw.prepare_ok(vec![meta("f", "truth.dng", bytes.len() as u64, Some(sha256_hex(&bytes).to_uppercase()))]).await;
    assert_eq!(raw.upload(&truth, "f", bytes.clone()).await, 200);
    assert_eq!(names(dst.path()), ["truth.dng"]);
}

/// A body that ends cleanly but short or long, with no checksum to catch it, is refused all the same.
#[tokio::test]
async fn a_body_of_the_wrong_size_is_refused_and_leaves_nothing() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, mut events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());

    for (name, sent) in [("short.dng", 5usize), ("long.dng", 15)] {
        let session = raw.prepare_ok(vec![meta("f", name, 10, None)]).await;
        assert_eq!(raw.upload(&session, "f", vec![1u8; sent]).await, 400, "{name}");
        let reason = wait_for(&mut events, |e| match e {
            Event::FileFailed { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .await;
        assert!(reason.contains("announced"), "{reason}");
        wait_for(&mut events, |e| matches!(e, Event::SessionFinished { .. }).then_some(())).await;
        assert!(names(dst.path()).is_empty(), "nothing written: {:?}", names(dst.path()));
    }

    // The announced size goes through, so the refusals above are the size check's doing.
    let session = raw.prepare_ok(vec![meta("f", "exact.dng", 10, None)]).await;
    assert_eq!(raw.upload(&session, "f", vec![1u8; 10]).await, 200);
    assert_eq!(names(dst.path()), ["exact.dng"]);
}

#[tokio::test]
async fn traversal_names_land_as_plain_names_inside_the_destination() {
    let root = tempfile::tempdir().unwrap();
    let dst = root.path().join("inbox").join("deep");
    let (receiver, _events) = start(&dst, |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let names_in = ["../../evil.txt", "/etc/passwd", "..\\..\\win.ini", ".."];
    let files =
        names_in.iter().enumerate().map(|(i, n)| meta(&i.to_string(), n, 3, Some(sha256_hex(b"bad")))).collect();
    let session = raw.prepare_ok(files).await;
    for i in 0..names_in.len() {
        assert_eq!(raw.upload(&session, &i.to_string(), b"bad".to_vec()).await, 200);
    }
    assert_eq!(names(&dst), ["evil.txt", "file", "passwd", "win.ini"]);
    assert_eq!(names(root.path()), ["inbox"]);
    assert_eq!(names(&root.path().join("inbox")), ["deep"]);
}

#[tokio::test]
async fn a_sidecar_or_a_roll_marker_sent_beside_an_original_is_left_out() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, _events) = start(dst.path(), |_| {}).await;
    let planted = [write(src.path(), "DSC_0001.NEF.json", b"{}"), write(src.path(), ".oni-roll.json", b"{}")];
    let report = send_files(&me("S"), &target(&receiver), &planted, None).await.unwrap();
    assert_eq!((report.session_id, report.skipped.len()), (None, 2), "nothing to receive");
    assert!(names(dst.path()).is_empty());
    // Twin: the original beside them is received, and only it.
    let original = write(src.path(), "DSC_0001.NEF", b"raw");
    let all = [planted[0].clone(), original.clone(), planted[1].clone()];
    let report = send_files(&me("S"), &target(&receiver), &all, None).await.unwrap();
    assert_eq!(report.sent, vec![original]);
    assert_eq!(names(dst.path()), ["DSC_0001.NEF"]);
}

#[tokio::test]
async fn a_second_file_of_the_same_name_gets_a_suffix() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, _events) = start(dst.path(), |_| {}).await;
    let first = write(src.path(), "a.dng", b"first");
    send_files(&me("S"), &target(&receiver), std::slice::from_ref(&first), None).await.unwrap();
    std::fs::write(&first, b"second").unwrap();
    send_files(&me("S"), &target(&receiver), &[first], None).await.unwrap();
    assert_eq!(names(dst.path()), ["a (1).dng", "a.dng"]);
    assert_eq!(std::fs::read(dst.path().join("a.dng")).unwrap(), b"first");
    assert_eq!(std::fs::read(dst.path().join("a (1).dng")).unwrap(), b"second");
}

#[tokio::test]
async fn a_pin_is_required_when_set() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, _events) = start(dst.path(), |c| c.pin = Some("123456".into())).await;
    let file = vec![write(src.path(), "x.dng", b"x")];
    let raw = Raw::new(receiver.local_addr());
    assert_eq!(raw.prepare(vec![meta("f", "x", 1, None)], None).await.status(), 401);
    assert_eq!(raw.prepare(vec![meta("f", "x", 1, None)], Some("000000")).await.status(), 401);

    let t = target(&receiver);
    assert!(matches!(send_files(&me("S"), &t, &file, None).await, Err(SendError::PinRequired)));
    assert!(matches!(send_files(&me("S"), &t, &file, Some("654321")).await, Err(SendError::PinRequired)));
    assert!(names(dst.path()).is_empty());
    send_files(&me("S"), &t, &file, Some("123456")).await.unwrap();
    assert_eq!(names(dst.path()), ["x.dng"]);
}

#[tokio::test]
async fn a_rejecting_policy_answers_403_and_writes_nothing() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, mut events) = start(dst.path(), |c| c.policy = Arc::new(RejectAll)).await;
    let file = vec![write(src.path(), "x.dng", b"x")];
    assert!(matches!(send_files(&me("S"), &target(&receiver), &file, None).await, Err(SendError::Rejected)));
    assert_eq!(Raw::new(receiver.local_addr()).prepare(vec![meta("f", "x", 1, None)], None).await.status(), 403);
    assert!(names(dst.path()).is_empty());
    assert!(events.try_recv().is_err(), "no session started");
}

#[tokio::test]
async fn known_fingerprints_are_accepted_and_others_rejected() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let known = HashSet::from(["ABCDEF".to_string()]);
    let (receiver, _events) = start(dst.path(), |c| c.policy = Arc::new(AcceptFingerprints(known))).await;
    let file = vec![write(src.path(), "x.dng", b"x")];
    let t = target(&receiver);
    assert!(matches!(send_files(&me("STRANGER"), &t, &file, None).await, Err(SendError::Rejected)));
    assert!(names(dst.path()).is_empty());
    send_files(&me("abcdef"), &t, &file, None).await.unwrap();
    assert_eq!(names(dst.path()), ["x.dng"]);
}

#[tokio::test]
async fn a_partial_acceptance_sends_only_the_chosen_files() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, _events) = start(dst.path(), |c| c.policy = Arc::new(OnlyDng)).await;
    let files = vec![write(src.path(), "a.dng", b"a"), write(src.path(), "b.jpg", b"b")];
    let report = send_files(&me("S"), &target(&receiver), &files, None).await.unwrap();
    assert_eq!((report.sent, report.skipped), (vec![files[0].clone()], vec![files[1].clone()]));
    assert_eq!(names(dst.path()), ["a.dng"]);
}

#[tokio::test]
async fn a_second_session_is_blocked_while_one_is_active() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, _events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let first = raw.prepare_ok(vec![meta("f", "a", 1, None)]).await;
    assert_eq!(raw.prepare(vec![meta("g", "b", 1, None)], None).await.status(), 409);
    assert_eq!(raw.upload(&first, "f", b"a".to_vec()).await, 200);
    // The session ended with its last file, so the slot is free again.
    raw.prepare_ok(vec![meta("g", "b", 1, None)]).await;
}

#[tokio::test]
async fn upload_checks_session_token_and_file() {
    let dst = tempfile::tempdir().unwrap();
    let (receiver, _events) = start(dst.path(), |_| {}).await;
    let raw = Raw::new(receiver.local_addr());
    let session = raw.prepare_ok(vec![meta("f", "a", 1, None)]).await;
    let url = format!("https://{}/api/localsend/v2/upload", receiver.local_addr());
    let status = |q: String| {
        let (client, url) = (raw.client.clone(), url.clone());
        async move { client.post(format!("{url}?{q}")).body("a").send().await.unwrap().status().as_u16() }
    };
    let (s, t) = (&session.session_id, &session.files["f"]);
    assert_eq!(status(format!("sessionId={s}&fileId=f")).await, 400);
    assert_eq!(status(format!("sessionId={s}&fileId=f&token=wrong")).await, 403);
    assert_eq!(status(format!("sessionId={s}&fileId=nope&token={t}")).await, 403);
    assert_eq!(status(format!("sessionId=other&fileId=f&token={t}")).await, 409);
    assert!(names(dst.path()).is_empty());
    assert_eq!(status(format!("sessionId={s}&fileId=f&token={t}")).await, 200);
    assert_eq!(status(format!("sessionId={s}&fileId=f&token={t}")).await, 403, "session over");
}

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

#[tokio::test]
async fn a_wrong_pinned_fingerprint_fails_before_anything_is_sent() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, mut events) = start(dst.path(), |_| {}).await;
    let file = vec![write(src.path(), "x.dng", b"x")];
    let mut t = target(&receiver);

    t.fingerprint = Some("00".repeat(32));
    let err = send_files(&me("S"), &t, &file, None).await.unwrap_err();
    assert!(matches!(err, SendError::FingerprintMismatch), "{err:?}");
    assert!(matches!(register(&me("S"), &t).await, Err(SendError::FingerprintMismatch)));
    assert!(names(dst.path()).is_empty());
    assert!(events.try_recv().is_err(), "the receiver saw no session");

    // The true fingerprint, in either case, goes through.
    t.fingerprint = Some(receiver.info().fingerprint.to_lowercase());
    assert_eq!(register(&me("S"), &t).await.unwrap().fingerprint, receiver.info().fingerprint);
    send_files(&me("S"), &t, &file, None).await.unwrap();
    assert_eq!(names(dst.path()), ["x.dng"]);
}

#[tokio::test]
async fn info_and_register_answer_with_our_device() {
    let dst = tempfile::tempdir().unwrap();
    let identity = Identity::generate().unwrap();
    let fingerprint = identity.fingerprint().to_string();
    let peers = Peers::new(fingerprint.clone());
    let peers_for_receiver = peers.clone();
    let (receiver, _events) = start(dst.path(), move |c| {
        c.identity = identity;
        c.peers = Some(peers_for_receiver);
    })
    .await;
    let addr = receiver.local_addr();
    let client = https_client(None).unwrap();

    let info: DeviceInfo =
        client.get(format!("https://{addr}/api/localsend/v2/info")).send().await.unwrap().json().await.unwrap();
    assert_eq!((info.alias.as_str(), info.port, info.fingerprint.as_str()), ("ONI", addr.port(), fingerprint.as_str()));
    assert_eq!(info.version, PROTOCOL_VERSION);

    let answer = register(&me("PHONE"), &target(&receiver)).await.unwrap();
    assert_eq!(answer, info);
    let listed = peers.list();
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].info.fingerprint.as_str(), listed[0].ip), ("PHONE", addr.ip()));
}

#[tokio::test]
async fn a_policy_sees_the_pin_and_may_trust_it() {
    let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (receiver, _events) = start(dst.path(), |c| c.policy = Arc::new(OnlyPin)).await;
    let file = vec![write(src.path(), "x.dng", b"x")];
    let t = target(&receiver);
    assert!(matches!(send_files(&me("S"), &t, &file, None).await, Err(SendError::Rejected)));
    assert!(matches!(send_files(&me("S"), &t, &file, Some("guessed")).await, Err(SendError::Rejected)));
    assert!(names(dst.path()).is_empty());
    send_files(&me("S"), &t, &file, Some("handed-out")).await.unwrap();
    assert_eq!(names(dst.path()), ["x.dng"]);
}

#[test]
fn sending_can_run_on_any_runtime_thread() {
    fn spawnable<F: std::future::Future + Send>(_: F) {}
    let (me, target) = (me("S"), Target::new("127.0.0.1:1".parse().unwrap()));
    spawnable(send_files(&me, &target, &[], None));
}
