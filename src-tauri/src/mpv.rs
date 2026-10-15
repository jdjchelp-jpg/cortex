//! Ad-free YouTube (and arbitrary URL) audio playback via a headless `mpv`
//! sidecar.
//!
//! Why mpv: it plays both normal videos and livestreams, handles the HLS that
//! YouTube livestreams resolve to, and — fed through `yt-dlp` — bypasses the
//! ad-insertion that an embedded YouTube player would show. We run ONE long-
//! lived `mpv --idle` process with no video output and drive it over its JSON
//! IPC socket (load a URL, pause/resume, set volume). The process outlives
//! individual commands so playback is continuous across the app.
//!
//! `yt-dlp` is auto-downloaded (standalone Linux binary) into the app data dir
//! on first use, so the user doesn't have to install it manually. `mpv` and
//! `ffmpeg` are expected on PATH (detected + surfaced in Settings).

use crate::error::{Error, Result};
use crate::models::MediaTools;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// The single long-lived mpv child process, if running.
fn mpv_holder() -> &'static Mutex<Option<Child>> {
    static MPV: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    MPV.get_or_init(|| Mutex::new(None))
}

/// Kill the mpv sidecar — called on app exit so no headless mpv process lingers
/// (on Linux a child isn't auto-reaped when the parent exits).
pub fn shutdown() {
    if let Ok(mut guard) = mpv_holder().lock() {
        if let Some(child) = guard.as_mut() {
            // mpv is spawned as its own process-group leader (see ensure_mpv), so
            // for a YouTube livestream the continuously-running `yt-dlp` feeder
            // shares mpv's group. SIGKILL the whole group so the downloader can't
            // linger (and keep streaming) after the app closes — killing mpv alone
            // would orphan yt-dlp.
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }
}

fn data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| Error::Other(e.to_string()))
}

/// Quit any mpv orphaned by a previous session. If the app crashed, shutdown()
/// never ran — the old mpv (own process group) keeps streaming forever, and a
/// fresh launch used to delete its socket and spawn a SECOND mpv next to it.
/// A live listener on the leftover socket can only be that orphan: tell it to
/// quit before clearing the socket. Called once at app startup.
pub fn cleanup_stale(app: &AppHandle) {
    let Ok(dir) = data_dir(app) else { return };
    let socket = socket_path(&dir);
    let _ = ipc(&socket, &json!({ "command": ["quit"] }));
    #[cfg(not(windows))]
    { let _ = std::fs::remove_file(&socket); }
}

fn socket_path(dir: &Path) -> String {
    #[cfg(windows)]
    {
        // mpv on Windows exposes IPC over a Win32 NAMED PIPE — never a Unix
        // socket and never a TCP port (see mpv's input/ipc-win.c, which prepends
        // `\\.\pipe\` to the value). Passing a socket path or a `host:port` here
        // is what produced "unsupported: URL audio streaming requires a Unix
        // socket." A named pipe is opened through the file API (CreateFileW),
        // which `std::fs::OpenOptions` already uses — so no extra dependency.
        //
        // Name it after this install's data dir: stable across sessions (so
        // cleanup_stale can still reach an mpv orphaned by a crashed run) but
        // namespaced rather than a guessable global pipe. The OS destroys the
        // pipe automatically when the mpv process exits, so there's no file to
        // clean up on Windows.
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        dir.hash(&mut h);
        format!(r"\\.\pipe\cortex-mpv-{:016x}", h.finish())
    }
    #[cfg(not(windows))]
    { dir.join("mpv.sock").display().to_string() }
}

/// Resolve `bin` against PATH, returning its absolute path if found.
fn find_on_path(bin: &str) -> Option<PathBuf> {
    // Delegate to ingest::which so mpv gets the same Homebrew / minimal-PATH fallback
    // dirs (a Finder-launched macOS app omits /opt/homebrew/bin from PATH).
    crate::ingest::which(bin).map(PathBuf::from)
}

/// Ensure a usable `yt-dlp` exists, returning its path. Prefers an existing
/// install (downloaded copy, then PATH); otherwise downloads the standalone
/// Linux binary into `<data>/bin/yt-dlp`. Idempotent.
fn ensure_ytdlp(dir: &Path) -> Result<PathBuf> {
    // Bundled sidecar (shipped with the app) takes priority — no download.
    if let Some(p) = crate::ingest::bundled("yt-dlp") {
        return Ok(PathBuf::from(p));
    }
    #[cfg(windows)]
    let downloaded = dir.join("bin").join("yt-dlp.exe");
    #[cfg(not(windows))]
    let downloaded = dir.join("bin").join("yt-dlp");
    if downloaded.is_file() {
        return Ok(downloaded);
    }
    if let Some(p) = find_on_path("yt-dlp") {
        return Ok(p);
    }
    // Download the self-contained build (no system Python needed).
    #[cfg(windows)]
    let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
    #[cfg(target_os = "macos")]
    let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
    #[cfg(all(unix, not(target_os = "macos")))]
    let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
    std::fs::create_dir_all(downloaded.parent().unwrap())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|e| Error::Other(format!("yt-dlp download failed: {e}")))?
        .error_for_status()
        .map_err(|e| Error::Other(format!("yt-dlp download failed: {e}")))?
        .bytes()?;
    std::fs::write(&downloaded, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&downloaded)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&downloaded, perms)?;
    }
    Ok(downloaded)
}

/// Start mpv (idle, no video) if it isn't already running, listening on the IPC
/// socket and configured to find our yt-dlp. Waits briefly for the socket.
fn ensure_mpv(socket: &str, ytdlp: &Path, volume: u8) -> Result<()> {
    let holder = mpv_holder();
    let mut guard = holder.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        // Still alive? Reuse it.
        if matches!(child.try_wait(), Ok(None)) {
            return Ok(());
        }
        *guard = None;
    }
    // A leftover socket with a live listener is an orphan from a crashed
    // session — quit it so two mpvs never play at once, then clear the socket.
    let _ = ipc(socket, &json!({ "command": ["quit"] }));
    #[cfg(not(windows))]
    { let _ = std::fs::remove_file(socket); }
    let mpv_bin = find_on_path("mpv").unwrap_or_else(|| PathBuf::from("mpv"));
    let mut cmd = Command::new(&mpv_bin);
    cmd.arg("--no-video")
        .arg("--idle=yes")
        .arg("--no-terminal")
        .arg("--really-quiet")
        // Buffer ahead so a resolved direct stream starts (and stays) smooth.
        .arg("--cache=yes")
        .arg("--demuxer-readahead-secs=10")
        .arg(format!("--volume={volume}"))
        .arg(format!("--input-ipc-server={socket}"))
        .arg(format!(
            "--script-opts=ytdl_hook-ytdl_path={}",
            ytdlp.display()
        ));
    // Put mpv in its own process group so any `yt-dlp` feeder it spawns shares the
    // group and can be torn down together on shutdown() (see there).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| {
        Error::Other(format!(
            "couldn't start mpv — install it (e.g. `sudo pacman -S mpv`): {e}"
        ))
    })?;
    *guard = Some(child);
    drop(guard);
    for _ in 0..60 {
        if ipc(socket, &json!({ "command": ["get_property", "idle-active"] })).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// Send one JSON command line to mpv's IPC socket.
#[cfg(unix)]
fn ipc(socket: &str, cmd: &Value) -> Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(socket)
        .map_err(|e| Error::Other(format!("mpv not reachable: {e}")))?;
    let mut line = serde_json::to_vec(cmd)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    Ok(())
}

/// Send one JSON command line to mpv's IPC endpoint.
///
/// Windows: mpv listens on a Win32 named pipe (see `socket_path`). A named pipe
/// is opened like a file through CreateFileW — exactly what `OpenOptions::open`
/// does — so we connect by opening the pipe path and writing the command line.
/// This is a SINGLE attempt, mirroring the Unix `UnixStream::connect`: callers
/// that must wait for mpv to come up (`ensure_mpv`) poll in a loop, so a direct
/// call when mpv isn't running fails fast (keeping startup/`cleanup_stale`
/// instant) instead of blocking.
#[cfg(windows)]
fn ipc(socket: &str, cmd: &Value) -> Result<()> {
    use std::io::Write;
    let mut stream = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(socket)
        .map_err(|e| Error::Other(format!("mpv not reachable on {socket}: {e}")))?;
    let mut line = serde_json::to_vec(cmd)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    Ok(())
}

/// Best-effort IPC (used for pause/stop/volume): if mpv isn't running, there's
/// nothing to control, so a connect failure is silently ignored.
fn ipc_soft(socket: &str, cmd: &Value) {
    let _ = ipc(socket, cmd);
}

// ---- commands ----------------------------------------------------------

/// Detect the external tools the YouTube-audio engine relies on.
#[tauri::command]
pub fn media_tools_status(app: AppHandle) -> Result<MediaTools> {
    let dir = data_dir(&app)?;
    let downloaded = dir.join("bin").join("yt-dlp");
    Ok(MediaTools {
        mpv: find_on_path("mpv").is_some(),
        ffmpeg: find_on_path("ffmpeg").is_some(),
        ytdlp: crate::ingest::bundled("yt-dlp").is_some() || downloaded.is_file() || find_on_path("yt-dlp").is_some(),
        ytdlp_path: downloaded.display().to_string(),
    })
}

/// Stream a URL's audio (YouTube video/livestream or any mpv-playable URL),
/// ad-free, starting playback at `volume` (0–100). Downloads yt-dlp on first use.
#[tauri::command]
pub fn youtube_play(app: AppHandle, url: String, volume: u8) -> Result<()> {
    let dir = data_dir(&app)?;
    // One unified path on every OS: a single long-lived `mpv --idle` driven over
    // its IPC endpoint (Unix socket on Linux/macOS, named pipe on Windows — see
    // socket_path/ipc). Opening the URL is just a `loadfile` command over IPC; no
    // Unix socket is required merely to play a URL. This keeps play/pause/resume/
    // stop/volume working identically everywhere.
    let socket = socket_path(&dir);
    let ytdlp = ensure_ytdlp(&dir)?;
    ensure_mpv(&socket, &ytdlp, volume)?;
    // Stations are YouTube watch URLs; mpv's ytdl hook normally re-runs yt-dlp on
    // every load to resolve the direct googlevideo stream (several seconds). We
    // cache that resolved URL per station so repeat plays start near-instantly.
    // Playlist/radio URLs are skipped (they need the hook for playlist behaviour).
    let loaded_url = match cache_lookup_fresh(&dir, &url) {
        // FRESH hit: hand mpv the direct stream — no yt-dlp on the play path.
        Some(direct) => {
            ipc(
                &socket,
                &json!({ "command": ["loadfile", direct, "replace"] }),
            )?;
            // Re-resolve in the background so the cached URL stays fresh for next time.
            spawn_resolve_and_cache(dir.clone(), ytdlp.clone(), url.clone());
            true
        }
        None => false,
    };
    if !loaded_url {
        // Miss: fall back to the original URL (mpv's ytdl hook resolves it — the
        // user waits no longer than before) and prime the cache so the SECOND play
        // is instant. This also auto-covers user-added stations on first play.
        ipc(
            &socket,
            &json!({ "command": ["loadfile", url, "replace"] }),
        )?;
        spawn_resolve_and_cache(dir.clone(), ytdlp.clone(), url.clone());
    }
    ipc(&socket, &json!({ "command": ["set_property", "pause", false] }))?;
    ipc(
        &socket,
        &json!({ "command": ["set_property", "volume", volume as i64] }),
    )?;
    Ok(())
}

#[tauri::command]
pub fn youtube_pause(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(&socket_path(&dir), &json!({ "command": ["set_property", "pause", true] }));
    Ok(())
}

#[tauri::command]
pub fn youtube_resume(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(&socket_path(&dir), &json!({ "command": ["set_property", "pause", false] }));
    Ok(())
}

/// Stop playback but keep mpv idle (ready for the next station).
#[tauri::command]
pub fn youtube_stop(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(&socket_path(&dir), &json!({ "command": ["stop"] }));
    Ok(())
}

#[tauri::command]
pub fn youtube_set_volume(app: AppHandle, volume: u8) -> Result<()> {
    let dir = data_dir(&app)?;
    ipc_soft(
        &socket_path(&dir),
        &json!({ "command": ["set_property", "volume", volume as i64] }),
    );
    Ok(())
}

/// Pre-resolve a batch of station URLs in the background so their first play is
/// instant. Returns immediately; ONE detached thread walks the list, resolving
/// only stale/missing non-playlist entries (cheap no-op when all are fresh).
/// Best-effort: any failure (no yt-dlp, no network) just leaves the cache as-is.
#[tauri::command]
pub fn youtube_prewarm(app: AppHandle, urls: Vec<String>) -> Result<()> {
    let dir = data_dir(&app)?;
    // Resolve yt-dlp's path on the calling thread (cheap: avoids a download race
    // inside the worker); if it isn't available yet, skip prewarming entirely.
    let Ok(ytdlp) = ensure_ytdlp(&dir) else {
        return Ok(());
    };
    std::thread::spawn(move || {
        for url in urls {
            if is_playlist_url(&url) {
                continue; // playlists use mpv's ytdl hook; never cached
            }
            if cache_lookup_fresh(&dir, &url).is_some() {
                continue; // already fresh — nothing to do
            }
            if let Some(direct) = resolve_direct(&ytdlp, &url) {
                cache_store(&dir, &url, &direct);
            }
        }
    });
    Ok(())
}

// ---- resolved-stream cache --------------------------------------------------
//
// Each station URL is a YouTube watch page; mpv's ytdl hook shells out to yt-dlp
// to turn it into a direct googlevideo stream on every load (several seconds).
// We persist that resolved URL so repeat plays skip yt-dlp. Direct URLs expire
// (~6h) and can be IP-bound, so entries are only trusted for 3 hours.

/// How long a cached direct URL is trusted before we re-resolve it.
const CACHE_FRESH_MS: i64 = 3 * 60 * 60 * 1000; // 3 hours

/// Playlist/radio URLs (`list=`) must keep going through mpv's ytdl hook so
/// playlist/next-track behaviour works — never cache or resolve these.
fn is_playlist_url(url: &str) -> bool {
    url.contains("list=")
}

fn stream_cache_path(dir: &Path) -> PathBuf {
    dir.join("stream-cache.json")
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// One cached entry: a resolved direct stream URL and when it was resolved.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CacheEntry {
    direct: String,
    resolved_at_ms: i64,
}

/// Load the whole cache. A missing or corrupt file yields an empty map — the
/// cache is a pure optimisation and must never be a failure path.
fn cache_load(dir: &Path) -> std::collections::HashMap<String, CacheEntry> {
    std::fs::read_to_string(stream_cache_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist the cache best-effort; write errors are intentionally ignored.
fn cache_save(dir: &Path, map: &std::collections::HashMap<String, CacheEntry>) {
    if let Ok(s) = serde_json::to_string(map) {
        let _ = std::fs::write(stream_cache_path(dir), s);
    }
}

/// Return the cached direct URL for `url` only if it's a non-playlist entry that
/// resolved within the freshness window; otherwise `None`.
fn cache_lookup_fresh(dir: &Path, url: &str) -> Option<String> {
    if is_playlist_url(url) {
        return None;
    }
    let entry = cache_load(dir).remove(url)?;
    if now_ms().saturating_sub(entry.resolved_at_ms) < CACHE_FRESH_MS {
        Some(entry.direct)
    } else {
        None
    }
}

/// Insert/refresh one entry (skipping playlist URLs), preserving the rest.
fn cache_store(dir: &Path, url: &str, direct: &str) {
    if is_playlist_url(url) {
        return;
    }
    let mut map = cache_load(dir);
    map.insert(
        url.to_string(),
        CacheEntry {
            direct: direct.to_string(),
            resolved_at_ms: now_ms(),
        },
    );
    cache_save(dir, &map);
}

/// Resolve a YouTube/URL to its direct best-audio stream via yt-dlp's `-g`
/// (get-url). Returns the first stdout line, or `None` on any failure. Playlist
/// URLs return `None` (they must not be flattened to a single stream).
fn resolve_direct(ytdlp: &Path, url: &str) -> Option<String> {
    if is_playlist_url(url) {
        return None;
    }
    let out = Command::new(ytdlp)
        .arg("-g")
        .arg("-f")
        .arg("bestaudio")
        .arg("--no-playlist")
        .arg(url)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

/// Resolve `url` in a detached thread and cache the result. Used to prime the
/// cache after a miss and to refresh it after a fresh hit. Best-effort.
fn spawn_resolve_and_cache(dir: PathBuf, ytdlp: PathBuf, url: String) {
    if is_playlist_url(&url) {
        return;
    }
    std::thread::spawn(move || {
        if let Some(direct) = resolve_direct(&ytdlp, &url) {
            cache_store(&dir, &url, &direct);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression for the original Windows failure: mpv on Windows exposes IPC
    // over a Win32 named pipe, NOT a Unix socket and NOT a TCP port. The old
    // code passed a `host:port` (and drove it with TcpStream), so every control
    // silently failed and URL playback surfaced
    // "unsupported: URL audio streaming requires a Unix socket. (mpv)".
    // socket_path must therefore never return a socket path or a bare TCP
    // endpoint on Windows.
    #[cfg(windows)]
    #[test]
    fn windows_socket_path_is_a_named_pipe() {
        let dir = std::path::Path::new("C:\\Users\\test\\AppData\\Roaming\\cortex");
        let p = socket_path(dir);
        assert!(
            p.starts_with(r"\\.\pipe\"),
            "Windows IPC endpoint must be a named pipe, got: {p}"
        );
        assert!(!p.ends_with(".sock"), "must not be a Unix socket path: {p}");
        assert!(!p.contains(':'), "must not be a host:port TCP endpoint: {p}");
    }

    // The pipe name is derived from the data dir, so it's stable across calls
    // (cleanup_stale must reach an orphaned mpv) but scoped per install rather
    // than a guessable global name.
    #[cfg(windows)]
    #[test]
    fn windows_socket_path_is_stable_and_scoped() {
        let a = socket_path(std::path::Path::new("C:\\cortex-a"));
        let b = socket_path(std::path::Path::new("C:\\cortex-a"));
        let c = socket_path(std::path::Path::new("C:\\cortex-b"));
        assert_eq!(a, b, "same data dir must yield the same pipe name");
        assert_ne!(a, c, "different installs must not share a pipe name");
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_socket_path_is_a_socket_file() {
        let dir = std::path::Path::new("/home/test/.local/share/cortex");
        let p = socket_path(dir);
        assert!(p.ends_with("mpv.sock"), "Unix IPC endpoint must be a socket file, got: {p}");
        assert!(!p.contains(r"\\.\pipe\"), "Unix must not use a named pipe: {p}");
    }

    #[test]
    fn playlist_urls_are_never_cached() {
        assert!(is_playlist_url(
            "https://www.youtube.com/watch?v=x&list=PLabc"
        ));
        assert!(is_playlist_url(
            "https://www.youtube.com/watch?v=y&list=RDy&start_radio=1"
        ));
        assert!(!is_playlist_url("https://www.youtube.com/watch?v=z"));
    }

    #[test]
    fn store_then_fresh_lookup_roundtrips() {
        let dir = std::env::temp_dir().join(format!("cortex-stream-cache-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://www.youtube.com/watch?v=roundtrip";
        cache_store(&dir, url, "https://direct.example/stream.m4a");
        assert_eq!(
            cache_lookup_fresh(&dir, url).as_deref(),
            Some("https://direct.example/stream.m4a")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_entries_are_not_returned() {
        let dir = std::env::temp_dir().join(format!("cortex-stream-stale-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://www.youtube.com/watch?v=stale";
        let mut map = std::collections::HashMap::new();
        map.insert(
            url.to_string(),
            CacheEntry {
                direct: "https://direct.example/old.m4a".into(),
                resolved_at_ms: now_ms() - CACHE_FRESH_MS - 1,
            },
        );
        cache_save(&dir, &map);
        assert_eq!(cache_lookup_fresh(&dir, url), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_cache_file_yields_empty_map() {
        let dir = std::env::temp_dir().join(format!("cortex-stream-corrupt-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(stream_cache_path(&dir), "{ not json").unwrap();
        assert!(cache_load(&dir).is_empty());
        assert_eq!(cache_lookup_fresh(&dir, "https://x/watch?v=a"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn playlist_store_is_a_noop() {
        let dir = std::env::temp_dir().join(format!("cortex-stream-pl-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://www.youtube.com/watch?v=p&list=PLx";
        cache_store(&dir, url, "https://direct.example/should-not-store");
        assert!(cache_load(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
