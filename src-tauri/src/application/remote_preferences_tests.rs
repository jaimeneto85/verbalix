use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut header_end = None;
    let mut content_length = 0;
    loop {
        let mut chunk = [0_u8; 2048];
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if header_end.is_none() {
            header_end = bytes.windows(4).position(|window| window == b"\r\n\r\n");
            if let Some(end) = header_end {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or_default();
            }
        }
        if header_end.is_some_and(|end| bytes.len() >= end + 4 + content_length) {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    stream.flush().unwrap();
}

fn serve_preferences_api(
    requests_count: usize,
    handler: impl Fn(&str, &str) -> (&'static str, String) + Send + 'static,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let server = thread::spawn(move || {
        for _ in 0..requests_count {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let first_line = request.lines().next().unwrap_or_default().to_owned();
            recorded.lock().unwrap().push(request.clone());
            let (status, body) = handler(&first_line, &request);
            respond(&mut stream, status, &body);
        }
    });
    (format!("http://{address}"), requests, server)
}

fn base_settings() -> AppSettings {
    AppSettings {
        formality: 3,
        length: LengthPreference::Balanced,
        tone: TonePreference::Technical,
        confirm_before_replace: true,
        history_enabled: false,
        automatic_toolbar: true,
        shortcut: "Cmd+Shift+V".to_owned(),
    }
}

fn remote(formality: u8, updated_at: Option<&str>) -> RemotePreferences {
    RemotePreferences {
        formality,
        length: LengthPreference::Concise,
        tone: TonePreference::Friendly,
        history_enabled: true,
        updated_at: updated_at.map(|value| value.to_owned()),
    }
}

#[test]
fn merge_prefers_remote_when_updated_at_present() {
    let local = base_settings();
    let merged = merge_preferences(&local, Some(remote(5, Some("2026-07-25T00:00:00Z"))));

    assert_eq!(merged.formality, 5);
    assert_eq!(merged.length, LengthPreference::Concise);
    assert_eq!(merged.tone, TonePreference::Friendly);
    assert!(merged.history_enabled);
}

#[test]
fn merge_keeps_local_when_remote_updated_at_is_missing() {
    let local = base_settings();
    let merged = merge_preferences(&local, Some(remote(5, None)));

    assert_eq!(merged, local);
}

#[test]
fn merge_keeps_local_when_remote_row_is_absent() {
    let local = base_settings();
    let merged = merge_preferences(&local, None);

    assert_eq!(merged, local);
}

#[test]
fn merge_never_overwrites_macos_only_fields() {
    let local = base_settings();
    let merged = merge_preferences(&local, Some(remote(5, Some("2026-07-25T00:00:00Z"))));

    assert_eq!(merged.confirm_before_replace, local.confirm_before_replace);
    assert_eq!(merged.automatic_toolbar, local.automatic_toolbar);
    assert_eq!(merged.shortcut, local.shortcut);
}

#[test]
fn merge_never_overwrites_macos_only_fields_when_local_values_are_the_non_defaults() {
    let mut local = base_settings();
    local.confirm_before_replace = false;
    local.automatic_toolbar = false;
    local.shortcut = "Cmd+Shift+Z".to_owned();

    let merged = merge_preferences(&local, Some(remote(5, Some("2026-07-25T00:00:00Z"))));

    assert!(!merged.confirm_before_replace);
    assert!(!merged.automatic_toolbar);
    assert_eq!(merged.shortcut, "Cmd+Shift+Z");
}

#[tokio::test]
async fn fetch_returns_none_when_no_remote_row_exists() {
    let (base_url, _requests, server) =
        serve_preferences_api(1, |_first_line, _request| ("200 OK", "[]".to_owned()));
    let repository = RemotePreferencesRepository::new(base_url, "anonymous-key");

    let fetched = repository.fetch("access-token").await.unwrap();
    server.join().unwrap();

    assert!(fetched.is_none());
}

#[tokio::test]
async fn fetch_returns_remote_row_when_present() {
    let body = r#"[{"formality":4,"length":"detailed","tone":"assertive","history_enabled":true,"updated_at":"2026-07-25T00:00:00Z"}]"#;
    let (base_url, requests, server) =
        serve_preferences_api(1, move |_first_line, _request| ("200 OK", body.to_owned()));
    let repository = RemotePreferencesRepository::new(base_url, "anonymous-key");

    let fetched = repository.fetch("access-token").await.unwrap().unwrap();
    server.join().unwrap();

    assert_eq!(fetched.formality, 4);
    assert_eq!(fetched.length, LengthPreference::Detailed);
    assert_eq!(fetched.tone, TonePreference::Assertive);
    assert!(fetched.history_enabled);
    assert_eq!(fetched.updated_at.as_deref(), Some("2026-07-25T00:00:00Z"));
    let requests = requests.lock().unwrap();
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("apikey: anonymous-key"));
}

#[tokio::test]
async fn fetch_maps_unauthorized_to_unauthenticated() {
    let (base_url, _requests, server) = serve_preferences_api(1, |_first_line, _request| {
        ("401 Unauthorized", "{}".to_owned())
    });
    let repository = RemotePreferencesRepository::new(base_url, "anonymous-key");

    let error = repository.fetch("access-token").await.unwrap_err();
    server.join().unwrap();

    assert!(matches!(error, VerbalixError::Unauthenticated));
}

#[tokio::test]
async fn fetch_does_not_panic_on_unreachable_host_and_returns_an_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let repository = RemotePreferencesRepository::new(format!("http://{address}"), "anonymous-key");

    let error = repository.fetch("access-token").await.unwrap_err();

    assert!(matches!(error, VerbalixError::ProviderRejected));
}

#[tokio::test]
async fn upsert_sends_user_id_and_ai_fields_only() {
    let (base_url, requests, server) = serve_preferences_api(2, |first_line, _request| {
        if first_line.starts_with("GET /auth/v1/user ") {
            ("200 OK", r#"{"id":"user-1"}"#.to_owned())
        } else {
            ("201 Created", String::new())
        }
    });
    let repository = RemotePreferencesRepository::new(base_url, "anonymous-key");
    let settings = base_settings();

    repository.upsert(&settings, "access-token").await.unwrap();
    server.join().unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let upsert_request = requests
        .iter()
        .find(|request| request.starts_with("POST /rest/v1/user_preferences"))
        .unwrap();
    assert!(upsert_request.contains(r#""user_id":"user-1""#));
    assert!(upsert_request.contains(r#""formality":3"#));
    assert!(upsert_request.contains(r#""history_enabled":false"#));
    assert!(!upsert_request.contains("shortcut"));
    assert!(!upsert_request.contains("automatic_toolbar"));
    assert!(!upsert_request.contains("confirm_before_replace"));
    assert!(upsert_request
        .to_ascii_lowercase()
        .contains("prefer: resolution=merge-duplicates,return=minimal"));
}

#[tokio::test]
async fn upsert_maps_auth_lookup_failure_to_unauthenticated() {
    let (base_url, _requests, server) = serve_preferences_api(1, |_first_line, _request| {
        ("401 Unauthorized", "{}".to_owned())
    });
    let repository = RemotePreferencesRepository::new(base_url, "anonymous-key");
    let settings = base_settings();

    let error = repository
        .upsert(&settings, "access-token")
        .await
        .unwrap_err();
    server.join().unwrap();

    assert!(matches!(error, VerbalixError::Unauthenticated));
}

#[tokio::test]
async fn upsert_does_not_panic_on_unreachable_host_and_returns_an_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let repository = RemotePreferencesRepository::new(format!("http://{address}"), "anonymous-key");
    let settings = base_settings();

    let error = repository
        .upsert(&settings, "access-token")
        .await
        .unwrap_err();

    assert!(matches!(error, VerbalixError::ProviderRejected));
}
