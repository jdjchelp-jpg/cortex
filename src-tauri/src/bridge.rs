use crate::{commands, db::AppState, models::AddSourceInput, repo};
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::TcpListener;
use tauri::Manager;

pub fn start(app: &tauri::AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind("127.0.0.1:47821") else { return; };
        for stream in listener.incoming().flatten() {
            let handle = handle.clone();
            std::thread::spawn(move || {
                let mut stream = stream; let mut buf = [0u8; 2_000_000];
                let n = stream.read(&mut buf).unwrap_or(0); let req = String::from_utf8_lossy(&buf[..n]);
                let first = req.lines().next().unwrap_or("");
                let is_get = first.starts_with("GET /api/classroom/structure ");
                let is_post = first.starts_with("POST /api/classroom/import ");
                let ok = (is_get || is_post) && req.lines().any(|l| l.eq_ignore_ascii_case("x-cortex-bridge: cortex-local"));
                let payload = req.split("\r\n\r\n").nth(1).unwrap_or("");
                let (status, body) = if ok && is_post {
                    #[derive(Deserialize)] struct Import { subject: String, topic: String, path: String, name: String, kind: Option<String> }
                    match serde_json::from_str::<Import>(payload) {
                        Ok(item) => {
                            let input = handle.try_state::<AppState>().and_then(|s| s.db.lock().ok().and_then(|c| repo::list_subjects(&c).ok()).and_then(|subjects| {
                                let subject = subjects.into_iter().find(|s| s.name == item.subject)?;
                                let topic_id = subject.topics.iter().find(|t| t.name == item.topic).map(|t| t.id.clone());
                                Some(AddSourceInput { subject_id: subject.id, topic_id, name: Some(item.name), kind: item.kind, text: None, path: Some(item.path), url: None, tags: vec![] })
                            }));
                            let Some(input) = input else { return; };
                            match tauri::async_runtime::block_on(commands::add_source(handle.clone(), input)) {
                            Ok(result) => ("200 OK", serde_json::json!({"source":result.source}).to_string()),
                            Err(e) => ("422 Unprocessable Entity", serde_json::json!({"error":e.to_string()}).to_string()),
                            }
                        },
                        Err(e) => ("400 Bad Request", serde_json::json!({"error":e.to_string()}).to_string()),
                    }
                } else if ok { match handle.try_state::<AppState>() {
                    Some(s) => match s.db.lock().ok().and_then(|c| repo::list_subjects(&c).ok()) { Some(v) => ("200 OK", serde_json::json!({"subjects":v}).to_string()), None => ("500 Internal Server Error", "{\"error\":\"database unavailable\"}".into()) },
                    None => ("503 Service Unavailable", "{\"error\":\"Cortex is starting\"}".into()),
                }} else { ("403 Forbidden", "{\"error\":\"bridge header required\"}".into()) };
                let out = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()); let _ = stream.write_all(out.as_bytes());
            });
        }
    });
}
