use super::*;
use bytes::Bytes;
use rong_rt::http::{NetworkBodyChunk, NetworkOwner, NetworkRequestEnd, NetworkRequestId};
use std::io::Write as _;

const APP: &str = "app";

fn rid(id: u64) -> NetworkRequestId {
    NetworkRequestId::from_u64(id)
}

fn start_event(id: u64, owner: Option<&str>, kind: &str, url: &str) -> NetworkEvent {
    let kind = match kind {
        "fetch" => NetworkRequestKind::Fetch,
        "sse" => NetworkRequestKind::Sse,
        "download" => NetworkRequestKind::Download,
        _ => NetworkRequestKind::Other,
    };
    let mut start =
        NetworkRequestStart::new(rid(id), kind, http::Method::GET, url.parse().unwrap());
    start.owner = owner.map(NetworkOwner::new);
    start.headers = headers(&[("accept", "*/*"), ("authorization", "Bearer sk-live")]);
    NetworkEvent::RequestStart(start)
}

fn headers(pairs: &[(&str, &str)]) -> http::HeaderMap {
    pairs
        .iter()
        .map(|(name, value)| {
            (
                http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                http::HeaderValue::from_str(value).unwrap(),
            )
        })
        .collect()
}

fn fetch(state: &mut State, id: u64, url: &str) {
    state.apply(start_event(id, Some(APP), "fetch", url), false, 0);
}

fn head(state: &mut State, id: u64, status: u16, pairs: &[(&str, &str)]) {
    state.apply(
        NetworkEvent::ResponseHead(NetworkResponseHead::new(
            rid(id),
            http::StatusCode::from_u16(status).unwrap(),
            http::Version::HTTP_11,
            headers(pairs),
            Duration::from_millis(12),
        )),
        false,
        0,
    );
}

fn request_chunk(state: &mut State, id: u64, bytes: Bytes) {
    state.apply(
        NetworkEvent::RequestBodyChunk(NetworkBodyChunk::new(rid(id), bytes, Instant::now())),
        false,
        0,
    );
}

fn chunk_at(state: &mut State, id: u64, bytes: &[u8], at: Instant) {
    state.apply(
        NetworkEvent::ResponseBodyChunk(NetworkBodyChunk::new(
            rid(id),
            Bytes::copy_from_slice(bytes),
            at,
        )),
        false,
        0,
    );
}

fn chunk(state: &mut State, id: u64, bytes: &[u8]) {
    chunk_at(state, id, bytes, Instant::now());
}

fn end_event(id: u64, outcome: NetworkOutcome) -> NetworkEvent {
    NetworkEvent::RequestEnd(NetworkRequestEnd::new(
        rid(id),
        outcome,
        0,
        Duration::from_millis(30),
    ))
}

fn end(state: &mut State, id: u64, outcome: NetworkOutcome) {
    state.apply(end_event(id, outcome), false, 0);
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn recording(appid: Option<&str>) -> State {
    let mut state = State::new();
    state
        .start(DEV_SESSION_OWNER, appid.map(str::to_string), None)
        .unwrap();
    state
}

#[test]
fn a_record_follows_its_events_and_shows_a_gzip_body_decoded() {
    let mut state = recording(Some(APP));
    fetch(&mut state, 1, "https://api.test/me?token=abc");
    request_chunk(
        &mut state,
        1,
        Bytes::from_static(b"{\"password\":\"p\",\"q\":1}"),
    );
    // Events of an unknown request change nothing.
    chunk(&mut state, 99, b"stray");
    let body = br#"{"user":"ada","access_token":"secret"}"#;
    let wire = gzip(body);
    head(
        &mut state,
        1,
        200,
        &[
            ("content-type", "application/json"),
            ("content-encoding", "gzip"),
            ("set-cookie", "sid=1"),
        ],
    );
    chunk(&mut state, 1, &wire[..5]);
    chunk(&mut state, 1, &wire[5..]);
    end(&mut state, 1, NetworkOutcome::Completed);
    // A second end of the same request is ignored.
    end(&mut state, 1, NetworkOutcome::Failed("late".into()));

    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(stopped.requests.len(), 1);
    let record = &stopped.requests[0];
    assert_eq!(record["owner"], APP);
    assert_eq!(record["url"], "https://api.test/me?token=***");
    assert_eq!(record["status"], 200);
    assert_eq!(record["httpVersion"], "HTTP/1.1");
    assert_eq!(record["ttfbMs"], 12.0);
    assert_eq!(record["outcome"], "completed");
    assert_eq!(record["chunks"], 2);
    // Wire bytes are counted; the body is shown decoded and redacted.
    assert_eq!(record["responseBytes"], wire.len() as u64);
    assert_eq!(record["responseDecoded"], "gzip");
    assert_eq!(record["responseBody"]["user"], "ada");
    assert_eq!(record["responseBody"]["access_token"], "***");
    assert_eq!(record["requestBody"]["password"], "***");
    let headers = record["requestHeaders"].to_string();
    assert!(!headers.contains("sk-live"), "{headers}");
    assert!(
        record["responseHeaders"]
            .to_string()
            .contains("\"set-cookie\",\"***\"")
    );

    let scenario = stopped.scenario.to_scenario("rec");
    assert_eq!(
        scenario["rules"][0]["http"],
        "GET https://api.test/me?token=*"
    );
    assert_eq!(scenario["rules"][0]["json"]["user"], "ada");
    assert_eq!(scenario["rules"][0]["json"]["access_token"], "***");
}

#[test]
fn bodies_are_kept_within_the_caps() {
    let mut state = recording(None);
    let text = vec![b'a'; capture::MAX_TEXT_BODY_BYTES + 10];
    fetch(&mut state, 1, "https://a.test/text");
    head(&mut state, 1, 200, &[("content-type", "text/plain")]);
    chunk(&mut state, 1, &text);
    end(&mut state, 1, NetworkOutcome::Completed);

    let binary = vec![7u8; capture::MAX_BINARY_BODY_BYTES * 2];
    fetch(&mut state, 2, "https://a.test/blob");
    head(&mut state, 2, 200, &[("content-type", "image/png")]);
    chunk(&mut state, 2, &binary);
    end(&mut state, 2, NetworkOutcome::Completed);

    let request = vec![b'r'; registry::MAX_REQUEST_BODY_BYTES + 1];
    fetch(&mut state, 3, "https://a.test/upload");
    request_chunk(&mut state, 3, Bytes::from(request));
    head(&mut state, 3, 204, &[]);
    end(&mut state, 3, NetworkOutcome::Completed);

    let stored: Vec<usize> = state
        .dev
        .as_ref()
        .unwrap()
        .records
        .iter()
        .map(|record| record.response_body.len())
        .collect();
    assert_eq!(
        stored,
        vec![
            capture::MAX_TEXT_BODY_BYTES,
            capture::MAX_BINARY_BODY_BYTES,
            0
        ]
    );
    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(stopped.requests[0]["responseBodyTruncated"], true);
    assert_eq!(stopped.requests[1]["responseBodyTruncated"], true);
    // Valid UTF-8 of a binary type is still shown as bytes.
    assert!(stopped.requests[1]["responseBody"]["base64"].is_string());
    assert_eq!(stopped.requests[2]["requestBodyTruncated"], true);
    // A cut body is never replayed as if it were whole.
    let scenario = stopped.scenario.to_scenario("rec");
    let notes: Vec<_> = scenario["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| rule["note"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(notes[0].starts_with("text body of"), "{notes:?}");
    assert!(notes[1].starts_with("binary body of"), "{notes:?}");
    assert!(scenario["rules"][0].get("body").is_none());
}

#[test]
fn a_redirect_chain_is_linked_and_replays_as_one_answer() {
    let mut state = State::new();
    state
        .start(
            DEV_SESSION_OWNER,
            Some(APP.into()),
            Some(scenario::parse_url_string("**/start").unwrap()),
        )
        .unwrap();
    fetch(&mut state, 10, "https://a.test/start");
    head(&mut state, 10, 302, &[("location", "https://b.test/final")]);
    end(&mut state, 10, NetworkOutcome::Completed);
    // The hop's URL does not match the recording; it goes where its
    // predecessor went.
    let mut hop = start_event(11, Some(APP), "fetch", "https://b.test/final");
    if let NetworkEvent::RequestStart(start) = &mut hop {
        start.redirected_from = Some(rid(10));
    }
    state.apply(hop, false, 0);
    head(&mut state, 11, 200, &[("content-type", "text/plain")]);
    chunk(&mut state, 11, b"done");
    end(&mut state, 11, NetworkOutcome::Completed);

    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(stopped.requests.len(), 2);
    assert_eq!(stopped.requests[0]["status"], 302);
    assert_eq!(stopped.requests[1]["redirectedFrom"], 10);
    let scenario = stopped.scenario.to_scenario("rec");
    let rules = scenario["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1, "{scenario}");
    assert_eq!(rules[0]["http"], "GET https://a.test/start");
    assert_eq!(rules[0]["status"], 200);
    assert_eq!(rules[0]["body"], "done");
}

#[test]
fn blocked_failed_aborted_and_unread_requests() {
    let mut state = recording(Some(APP));
    fetch(&mut state, 1, "https://evil.test/x");
    end(
        &mut state,
        1,
        NetworkOutcome::Blocked("domain 'evil.test' is not allowed".into()),
    );
    fetch(&mut state, 2, "https://a.test/aborted");
    end(&mut state, 2, NetworkOutcome::Aborted);
    fetch(&mut state, 3, "https://a.test/unread");
    head(&mut state, 3, 200, &[("content-type", "text/plain")]);
    chunk(&mut state, 3, b"par");
    end(&mut state, 3, NetworkOutcome::Dropped);

    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(stopped.requests[0]["outcome"], "blocked");
    assert!(
        stopped.requests[0]["error"]
            .as_str()
            .unwrap()
            .contains("not allowed")
    );
    assert_eq!(stopped.requests[1]["outcome"], "aborted");
    assert_eq!(stopped.requests[2]["outcome"], "dropped");
    let scenario = stopped.scenario.to_scenario("rec");
    let rules = scenario["rules"].as_array().unwrap();
    // The app's own abort is not an answer.
    assert_eq!(rules.len(), 2, "{scenario}");
    assert_eq!(rules[0]["abort"], "failed");
    assert!(
        rules[1]["note"]
            .as_str()
            .unwrap()
            .contains("did not read the response body")
    );
}

#[test]
fn an_event_stream_keeps_its_frames_and_shows_in_flight() {
    let mut state = recording(Some(APP));
    let start = start_event(5, Some(APP), "sse", "https://a.test/events");
    let NetworkEvent::RequestStart(started) = &start else {
        unreachable!()
    };
    let t0 = started.started_at;
    state.apply(start, false, 0);
    head(&mut state, 5, 200, &[("content-type", "text/event-stream")]);
    // Chunk timings come from the event's own `at`, not from when it is applied.
    chunk_at(
        &mut state,
        5,
        b"event: tick\ndata: 1\n\n",
        t0 + Duration::from_millis(100),
    );
    chunk_at(
        &mut state,
        5,
        b"data: Bearer abc\r\nid: e2\r\n\r\n",
        t0 + Duration::from_millis(250),
    );

    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    let record = &stopped.requests[0];
    assert_eq!(record["outcome"], "inFlight");
    assert_eq!(record["kind"], "sse");
    assert_eq!(record["chunkLog"], json!([[100, 21], [250, 28]]));
    assert_eq!(
        record["events"],
        json!([
            { "event": "tick", "data": "1" },
            { "data": "Bearer ***", "id": "e2" }
        ])
    );
    // Not finished, so not an exchange.
    assert_eq!(stopped.scenario.dropped, 1);
    assert!(stopped.scenario.exchanges.is_empty());
    // The builder forgets the stream's bodies once nothing records it.
    assert!(state.in_flight[&5].response_body.is_empty());
}

#[test]
fn owners_decide_what_a_recording_keeps_and_replays() {
    let mut state = recording(Some(APP));
    state.apply(
        start_event(1, Some("other"), "fetch", "https://a.test/1"),
        false,
        0,
    );
    end(&mut state, 1, NetworkOutcome::Completed);
    fetch(&mut state, 2, "https://a.test/2");
    end(&mut state, 2, NetworkOutcome::Completed);
    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(stopped.requests.len(), 1);
    assert_eq!(stopped.requests[0]["id"], 2);

    // Every owner: all requests listed, only lxapp fetches replayable.
    let mut state = recording(None);
    for (id, owner, kind) in [
        (1, Some("host:update"), "download"),
        (2, None, "fetch"),
        (3, Some(APP), "download"),
        (4, Some(APP), "fetch"),
        (5, Some("host:registry"), "other"),
    ] {
        state.apply(start_event(id, owner, kind, "https://a.test/x"), false, 0);
        head(&mut state, id, 200, &[("content-type", "text/plain")]);
        end(&mut state, id, NetworkOutcome::Completed);
    }
    let stopped = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(stopped.requests.len(), 5);
    assert_eq!(stopped.scenario.exchanges.len(), 1);
    // Status lists every finished request without bodies.
    assert_eq!(state.recent.len(), 5);
    assert!(state.recent.iter().all(|r| r.response_body.is_empty()));
}

#[test]
fn a_dev_recording_pauses_while_a_run_records_its_own() {
    let mut state = recording(None);
    state.apply(
        start_event(1, Some(APP), "fetch", "https://a.test/before"),
        false,
        0,
    );
    // A run is active: the dev recording sees nothing of it.
    state.apply(
        start_event(2, Some(APP), "fetch", "https://a.test/unrecorded"),
        true,
        0,
    );
    state.start("run", None, None).unwrap();
    state.apply(
        start_event(3, Some(APP), "fetch", "https://a.test/during"),
        true,
        0,
    );
    for id in 1..=3 {
        end(&mut state, id, NetworkOutcome::Completed);
    }
    let run = state.stop("run").unwrap();
    assert_eq!(run.requests.len(), 1);
    assert_eq!(run.requests[0]["url"], "https://a.test/during");
    assert!(state.start("run-2", None, None).is_ok());
    assert!(state.start("run-3", None, None).is_err());
    let dev = state.stop(DEV_SESSION_OWNER).unwrap();
    assert_eq!(dev.requests.len(), 1);
    assert_eq!(dev.requests[0]["url"], "https://a.test/before");
}

#[test]
fn queue_overflow_marks_records_and_the_recent_list_is_bounded() {
    let mut state = State::new();
    state.apply(
        start_event(1, Some(APP), "fetch", "https://a.test/x"),
        false,
        3,
    );
    state.apply(end_event(1, NetworkOutcome::Completed), false, 4);
    assert_eq!(state.recent[0].summary_json()["incomplete"], true);
    for id in 2..(MAX_RECENT as u64 + 10) {
        fetch(&mut state, id, "https://a.test/y");
        end(&mut state, id, NetworkOutcome::Completed);
    }
    assert_eq!(state.recent.len(), MAX_RECENT);
}
