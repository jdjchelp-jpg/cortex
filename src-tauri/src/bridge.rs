use crate::{db::AppState, repo};
use std::io::{Read, Write};
use std::net::TcpListener;
use tauri::Manager;

pub fn start<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind("127.0.0.1:47821") else { return; };
        for stream in listener.incoming().flatten() {
            let handle = handle.clone();
            std::thread::spawn(move || {
                let mut stream = stream; let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0); let req = String::from_utf8_lossy(&buf[..n]);
                let ok = req.lines().next().map(|l| l.starts_with("GET /api/classroom/structure ")).unwrap_or(false) && req.lines().any(|l| l.eq_ignore_ascii_case("x-cortex-bridge: cortex-local"));
                let (status, body) = if ok { match handle.try_state::<AppState>() { Some(s) => match s.db.lock().ok().and_then(|c| repo::list_subjects(&c).ok()) { Some(v) => ("200 OK", serde_json::json!({"subjects":v}).to_string()), None => ("500 Internal Server Error", "{\"error\":\"database unavailable\"}".into()) }, None => ("503 Service Unavailable", "{\"error\":\"Cortex is starting\"}".into()) } } else { ("403 Forbidden", "{\"error\":\"bridge header required\"}".into()) };
                let out = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()); let _ = stream.write_all(out.as_bytes());
            });
        }
    });
}
