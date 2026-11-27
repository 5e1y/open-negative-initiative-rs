//! The upload API's routes and their handlers: register, info, prepare-upload, upload and cancel.

use super::*;

pub(super) fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/api/localsend/v2/register", post(register))
        .route("/api/localsend/v2/info", get(info))
        .route("/api/localsend/v2/prepare-upload", post(prepare_upload))
        .route("/api/localsend/v2/upload", post(upload))
        .route("/api/localsend/v2/cancel", post(cancel))
        .layer(DefaultBodyLimit::max(METADATA_LIMIT))
        .with_state(shared)
}

/// Frees a `Deciding` slot when the request ends without an answer, such as a sender hanging up.
struct DecidingGuard<'a>(&'a Shared);

impl Drop for DecidingGuard<'_> {
    fn drop(&mut self) {
        let mut slot = self.0.slot.lock().unwrap();
        if matches!(*slot, Slot::Deciding) {
            *slot = Slot::Free;
        }
    }
}

async fn register(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> Response {
    let Ok(info) = serde_json::from_slice::<DeviceInfo>(&body) else { return StatusCode::BAD_REQUEST.into_response() };
    if let Some(peers) = &shared.peers {
        peers.insert(info, peer.ip());
    }
    Json(&shared.info).into_response()
}

async fn info(State(shared): State<Arc<Shared>>) -> Response {
    Json(&shared.info).into_response()
}

#[derive(Deserialize)]
struct PinQuery {
    pin: Option<String>,
}

async fn prepare_upload(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<PinQuery>,
    body: Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<PrepareUploadRequest>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if shared.pin.is_some() && shared.pin != query.pin {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut files: Vec<FileMeta> = request.files.into_values().filter(|f| !refused_name(&f.file_name)).collect();
    if files.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }
    {
        let mut slot = shared.slot.lock().unwrap();
        if !matches!(*slot, Slot::Free) {
            return StatusCode::CONFLICT.into_response();
        }
        *slot = Slot::Deciding;
    }
    let guard = DecidingGuard(&shared);

    files.sort_by(|a, b| a.id.cmp(&b.id));
    let decided = tokio::time::timeout(DECIDE, shared.policy.decide(&request.info, &files, query.pin.as_deref())).await;
    match decided.unwrap_or(Decision::Reject) {
        Decision::Reject => return StatusCode::FORBIDDEN.into_response(),
        Decision::Accept(ids) => files.retain(|f| ids.contains(&f.id)),
        Decision::AcceptAll => {}
    }
    if files.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }

    let session = Arc::new(Session {
        id: crate::random_id(),
        sender_ip: peer.ip(),
        files: Mutex::new(
            files
                .iter()
                .map(|f| {
                    (f.id.clone(), FileState { meta: f.clone(), token: crate::random_id(), status: Status::Waiting })
                })
                .collect(),
        ),
        cancel: CancellationToken::new(),
        started: Instant::now(),
        last_activity_ms: AtomicU64::new(0),
    });
    let tokens = session.files.lock().unwrap().iter().map(|(id, f)| (id.clone(), f.token.clone())).collect();
    *shared.slot.lock().unwrap() = Slot::Active(session.clone());
    drop(guard);
    tokio::spawn(watch_idle(shared.clone(), session.clone()));
    shared.emit(Event::SessionStarted {
        session_id: session.id.clone(),
        sender: request.info,
        address: peer.ip(),
        files,
    });
    Json(PrepareUploadResponse { session_id: session.id.clone(), files: tokens }).into_response()
}

async fn watch_idle(shared: Arc<Shared>, session: Arc<Session>) {
    let tick = Duration::from_secs(1).min(SESSION_IDLE);
    loop {
        tokio::select! {
            _ = session.cancel.cancelled() => return,
            _ = tokio::time::sleep(tick) => {}
        }
        if shared.active(&session.id).is_none() {
            return;
        }
        if session.idle() >= SESSION_IDLE {
            tracing::info!("session {} idle, cancelling", session.id);
            shared.cancel(&session.id);
            return;
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadQuery {
    session_id: String,
    file_id: String,
    token: String,
}

async fn upload(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> StatusCode {
    let Some(session) = shared.active(&q.session_id) else {
        let busy = !matches!(*shared.slot.lock().unwrap(), Slot::Free);
        return if busy { StatusCode::CONFLICT } else { StatusCode::FORBIDDEN };
    };
    if session.sender_ip != peer.ip() {
        return StatusCode::FORBIDDEN;
    }
    let meta = {
        let mut files = session.files.lock().unwrap();
        let Some(file) = files.get_mut(&q.file_id).filter(|f| f.token == q.token) else {
            return StatusCode::FORBIDDEN;
        };
        if file.status != Status::Waiting {
            return StatusCode::CONFLICT;
        }
        file.status = Status::Receiving;
        file.meta.clone()
    };
    session.touch();

    let result = tokio::select! {
        r = receive(&shared, &session, &meta, body) => r,
        _ = session.cancel.cancelled() => return StatusCode::CONFLICT,
    };
    let (status, outcome) = match result {
        Ok(path) => {
            shared.emit(Event::FileDone { session_id: session.id.clone(), file_id: meta.id.clone(), path });
            (Status::Done, StatusCode::OK)
        }
        Err((code, reason)) => {
            shared.emit(Event::FileFailed { session_id: session.id.clone(), file_id: meta.id.clone(), reason });
            (Status::Failed, code)
        }
    };
    if let Some(f) = session.files.lock().unwrap().get_mut(&meta.id) {
        f.status = status;
    }
    shared.finish_if_complete(&session);
    outcome
}

async fn receive(
    shared: &Shared,
    session: &Session,
    meta: &FileMeta,
    body: Body,
) -> Result<PathBuf, (StatusCode, String)> {
    let io_error = |e: io::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    let mut part = PartFile::create(&shared.destination).await.map_err(io_error)?;
    let mut stream = body.into_data_stream();
    let mut reported = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        if part.written + chunk.len() as u64 > meta.size {
            return Err((StatusCode::BAD_REQUEST, "more bytes than announced".into()));
        }
        part.write(&chunk).await.map_err(io_error)?;
        session.touch();
        if part.written - reported >= PROGRESS_STEP {
            reported = part.written;
            shared.emit(Event::FileProgress {
                session_id: session.id.clone(),
                file_id: meta.id.clone(),
                received: part.written,
                size: meta.size,
            });
        }
    }
    // The checksum is optional in the protocol: the announced size is the guard every file carries.
    if part.written != meta.size {
        return Err((StatusCode::BAD_REQUEST, "fewer bytes than announced".into()));
    }
    let digest = part.seal().await.map_err(io_error)?;
    if let Some(expected) = &meta.sha256
        && !expected.eq_ignore_ascii_case(&digest)
    {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "sha256 mismatch".into()));
    }
    shared.emit(Event::FileProgress {
        session_id: session.id.clone(),
        file_id: meta.id.clone(),
        received: part.written,
        size: meta.size,
    });
    let name = sanitize_file_name(&meta.file_name);
    part.finish(Path::new(&shared.destination), &name, shared.rename_lock.clone()).await.map_err(io_error)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CancelQuery {
    session_id: String,
}

async fn cancel(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(q): Query<CancelQuery>,
) -> StatusCode {
    match shared.active(&q.session_id) {
        Some(s) if s.sender_ip == peer.ip() => {
            shared.cancel(&q.session_id);
            StatusCode::OK
        }
        Some(_) => StatusCode::FORBIDDEN,
        None => StatusCode::OK,
    }
}
