use super::*;
use crate::domain::{TransformOperation, TransformRequest, TransformResult};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use uuid::Uuid;

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

fn history_response() -> &'static str {
    r#"[
      {
        "id":"00000000-0000-0000-0000-000000000001",
        "operation":"translate",
        "source_text":"olá",
        "result_text":"hello",
        "source_language":"pt",
        "target_language":"en",
        "created_at":"2026-07-24T00:00:00Z"
      },
      {
        "id":"00000000-0000-0000-0000-000000000002",
        "operation":"improve",
        "source_text":"fix text",
        "result_text":"Improve this text",
        "source_language":"en",
        "target_language":null,
        "created_at":"2026-07-24T00:00:01Z"
      }
    ]"#
}

fn serve_history_api() -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let server = thread::spawn(move || {
        for _ in 0..5 {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let first_line = request.lines().next().unwrap_or_default().to_owned();
            recorded.lock().unwrap().push(request);
            if first_line.starts_with("GET /auth/v1/user ") {
                respond(&mut stream, "200 OK", r#"{"id":"user-1"}"#);
            } else if first_line.starts_with("POST /rest/v1/transform_history ") {
                respond(&mut stream, "201 Created", "");
            } else if first_line
                .starts_with("GET /rest/v1/transform_history?select=*&order=created_at.desc ")
            {
                respond(&mut stream, "200 OK", history_response());
            } else {
                respond(&mut stream, "404 Not Found", "{}");
            }
        }
    });
    (format!("http://{address}"), requests, server)
}

fn transformation(
    operation: TransformOperation,
    source: &str,
    result: &str,
) -> (TransformRequest, TransformResult) {
    let request_id = Uuid::new_v4();
    (
        TransformRequest {
            request_id,
            operation,
            text: source.to_owned(),
            preferences: None,
        },
        TransformResult {
            request_id,
            source_language: if operation == TransformOperation::Translate {
                "pt"
            } else {
                "en"
            }
            .to_owned(),
            target_language: (operation == TransformOperation::Translate).then(|| "en".to_owned()),
            result: result.to_owned(),
        },
    )
}

#[tokio::test]
async fn successful_translate_and_improve_are_inserted_and_listed() {
    let (base_url, requests, server) = serve_history_api();
    let repository = RemoteHistoryRepository::new(base_url, "anonymous-key");
    for (operation, source, result) in [
        (TransformOperation::Translate, "olá", "hello"),
        (TransformOperation::Improve, "fix text", "Improve this text"),
    ] {
        let (request, response) = transformation(operation, source, result);
        repository
            .insert(&request, &response, "access-token")
            .await
            .unwrap();
    }

    let history = repository.list("access-token").await.unwrap();
    server.join().unwrap();

    assert_eq!(
        history
            .iter()
            .map(|item| item.operation.as_str())
            .collect::<Vec<_>>(),
        ["translate", "improve"]
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("GET /auth/v1/user "))
            .count(),
        2
    );
    assert!(requests
        .iter()
        .any(|request| request.contains(r#""operation":"translate""#)));
    assert!(requests
        .iter()
        .any(|request| request.contains(r#""operation":"improve""#)));
    assert!(requests.iter().all(|request| {
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer access-token")
    }));
}
