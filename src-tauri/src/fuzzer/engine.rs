use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Semaphore};

use super::filters::FilterConfig;
use super::request::{build_request, FuzzConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzResult {
    pub word: String,
    pub url: String,
    pub status: u16,
    pub size: u64,
    pub words: u64,
    pub lines: u64,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FuzzProgress {
    pub done: usize,
    pub total: usize,
    pub matches: usize,
    pub rps: f64,
}

/// How often buffered results and progress are handed to the callbacks. Each
/// hand-off becomes an IPC event and a re-render, so per-result delivery would
/// swamp the webview on a fast scan; 10 updates a second still looks live.
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

/// Lets a Tauri command ("Stop" button) reach a run that's already in
/// flight. A plain `Arc<AtomicBool>` rather than a global/static: each run
/// gets its own handle, so there's nothing to reset between runs and no
/// unsafe global state.
#[derive(Clone)]
pub struct FuzzHandle {
    cancelled: Arc<AtomicBool>,
}

impl FuzzHandle {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// True if both handles belong to the same run (clones share the flag).
    pub fn same_run(&self, other: &FuzzHandle) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

/// Runs one fuzz job end to end.
///
/// `on_results` / `on_progress` are callbacks rather than a return value —
/// a Tauri command uses them to emit events to the frontend as responses come
/// back, so the results table fills in live instead of the whole run having
/// to finish (and sit buffered in memory) before the user sees a single row.
/// That matters once a wordlist runs into the tens of thousands of entries.
/// Both are called at most once per `FLUSH_INTERVAL`, plus once at the end.
///
/// Concurrency model: one spawned task per word, gated by a `Semaphore`
/// with `config.concurrency` permits — the direct equivalent of ffuf's
/// `-t` flag. A bounded `mpsc` channel carries finished results back to
/// this function's receive loop, which runs concurrently with the
/// producer (not after it), so the channel never has to hold more than a
/// few hundred results at once regardless of wordlist size.
pub async fn run_fuzz<FR, FP>(
    config: FuzzConfig,
    filters: FilterConfig,
    wordlist: Vec<String>,
    handle: FuzzHandle,
    mut on_results: FR,
    mut on_progress: FP,
) -> Result<(), String>
where
    FR: FnMut(Vec<FuzzResult>) + Send,
    FP: FnMut(FuzzProgress) + Send,
{
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(config.timeout_ms.max(1)))
        .redirect(if config.follow_redirects {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        })
        .build()
        .map_err(|e| format!("Failed to initialize HTTP client: {e}"))?;

    let method = Method::from_bytes(config.method.as_bytes())
        .map_err(|_| format!("Invalid HTTP method: {}", config.method))?;

    let total = wordlist.len();
    let semaphore = Arc::new(Semaphore::new(config.concurrency.max(1)));
    let (tx, mut rx) = mpsc::channel::<FuzzResult>(256);

    let client = Arc::new(client);
    let config = Arc::new(config);
    let start = Instant::now();

    // Producer: walks the wordlist, spawning one request task per word as
    // soon as a concurrency permit is free. Runs as its own task so it
    // proceeds independently of how fast the receive loop below drains
    // results — the two run concurrently rather than one blocking on the
    // other.
    tokio::spawn({
        let handle = handle.clone();
        let client = client.clone();
        let config = config.clone();
        let method = method.clone();
        let semaphore = semaphore.clone();
        async move {
            for word in wordlist.into_iter() {
                if handle.is_cancelled() {
                    break;
                }
                let Ok(permit) = semaphore.clone().acquire_owned().await else {
                    break;
                };
                let client = client.clone();
                let config = config.clone();
                let method = method.clone();
                let tx = tx.clone();
                let handle = handle.clone();

                tokio::spawn(async move {
                    let _permit = permit; // released when this task ends, freeing a slot
                    if handle.is_cancelled() {
                        return;
                    }

                    let built = build_request(&config, &word);
                    let req_start = Instant::now();

                    let mut req = client.request(method.clone(), built.url.as_str());
                    for (k, v) in &built.headers {
                        req = req.header(k.as_str(), v.as_str());
                    }
                    if let Some(body) = built.body.clone() {
                        req = req.body(body);
                    }

                    let result = match req.send().await {
                        Ok(mut resp) => {
                            let status = resp.status().as_u16();
                            // Streamed chunk by chunk rather than `bytes()`: a hostile
                            // server can send an endless body, and buffering it whole
                            // would exhaust memory. Only the counters are kept, so the
                            // full size is still reported, however large. The client
                            // timeout still bounds how long one body can take.
                            let mut stats = BodyStats::default();
                            let mut error = None;
                            loop {
                                match resp.chunk().await {
                                    Ok(Some(chunk)) => stats.feed(&chunk),
                                    Ok(None) => break,
                                    Err(e) => {
                                        error = Some(describe_error(&e, config.timeout_ms));
                                        break;
                                    }
                                }
                            }
                            stats.finish();
                            FuzzResult {
                                word: word.clone(),
                                url: built.url.clone(),
                                status,
                                size: stats.size,
                                words: stats.words,
                                lines: stats.lines(),
                                duration_ms: req_start.elapsed().as_millis() as u64,
                                error,
                            }
                        }
                        Err(e) => FuzzResult {
                            word: word.clone(),
                            url: built.url.clone(),
                            status: 0,
                            size: 0,
                            words: 0,
                            lines: 0,
                            duration_ms: req_start.elapsed().as_millis() as u64,
                            error: Some(describe_error(&e, config.timeout_ms)),
                        },
                    };

                    let _ = tx.send(result).await;

                    if config.delay_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(config.delay_ms))
                            .await;
                    }
                });
            }
            // `tx` (the original sender, captured by this async block) is
            // dropped here once every word has been scheduled. Combined
            // with every spawned task dropping its own clone when it
            // finishes, the channel closes — and the receive loop below
            // ends — only once all work is truly done.
        }
    });

    let mut done = 0usize;
    let mut matches = 0usize;
    let mut pending = Vec::new();
    let mut reported_done = 0usize;
    let mut flush = |pending: &mut Vec<FuzzResult>, done: usize, matches: usize| {
        if !pending.is_empty() {
            on_results(std::mem::take(pending));
        }
        if done != reported_done {
            reported_done = done;
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            on_progress(FuzzProgress {
                done,
                total,
                matches,
                rps: done as f64 / elapsed,
            });
        }
    };

    // The timer, not the next result, drives the flush: on a slow target the
    // next result can be seconds away, and what's buffered shouldn't wait for it.
    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    loop {
        tokio::select! {
            received = rx.recv() => match received {
                Some(result) => {
                    done += 1;
                    if filters.matches(&result) {
                        matches += 1;
                        pending.push(result);
                    }
                }
                None => break,
            },
            _ = ticker.tick() => flush(&mut pending, done, matches),
        }
    }
    flush(&mut pending, done, matches);

    Ok(())
}

/// Turns a reqwest error into something the user can act on. reqwest's own
/// `Display` is usually just "error sending request for url (...)", with the
/// actual cause (DNS failure, refused connection, TLS problem, ...) buried in
/// the `source()` chain, so this names the category and walks the chain.
fn describe_error(e: &reqwest::Error, timeout_ms: u64) -> String {
    let kind = if e.is_timeout() {
        format!("Timeout after {timeout_ms}ms")
    } else if e.is_connect() {
        "Connection failed".to_string()
    } else if e.is_redirect() {
        "Redirect error".to_string()
    } else if e.is_body() || e.is_decode() {
        "Response body could not be read".to_string()
    } else if e.is_builder() {
        "Invalid request".to_string()
    } else {
        "Request failed".to_string()
    };

    let mut causes: Vec<String> = Vec::new();
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        let text = cause.to_string();
        // Wrapping layers often repeat their inner error's message verbatim.
        if !causes.iter().any(|c| c.contains(&text) || text.contains(c.as_str())) {
            causes.push(text);
        }
        source = cause.source();
    }

    if causes.is_empty() {
        kind
    } else {
        format!("{kind}: {}", causes.join(" → "))
    }
}

/// Size / word / line counts of a response body, computed incrementally so
/// the body itself never has to be held in memory. Matches what
/// `String::from_utf8_lossy(&body)` followed by `split_whitespace().count()`
/// and `lines().count()` would give on the whole body, including when a
/// multi-byte UTF-8 character is split across two chunks.
#[derive(Default)]
struct BodyStats {
    size: u64,
    words: u64,
    newlines: u64,
    /// Last byte seen was not `\n`, i.e. there's an unterminated final line.
    trailing: bool,
    in_word: bool,
    /// Bytes of an incomplete UTF-8 sequence carried over to the next chunk.
    pending: Vec<u8>,
}

impl BodyStats {
    fn feed(&mut self, chunk: &[u8]) {
        self.size += chunk.len() as u64;
        // `\n` never occurs inside a multi-byte UTF-8 sequence, so lines can
        // be counted on raw bytes.
        self.newlines += chunk.iter().filter(|&&b| b == b'\n').count() as u64;
        if let Some(&last) = chunk.last() {
            self.trailing = last != b'\n';
        }

        let owned: Vec<u8>;
        let mut data: &[u8] = if self.pending.is_empty() {
            chunk
        } else {
            owned = {
                let mut p = std::mem::take(&mut self.pending);
                p.extend_from_slice(chunk);
                p
            };
            &owned
        };
        loop {
            match std::str::from_utf8(data) {
                Ok(s) => {
                    self.count_chars(s);
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    // Safe to unwrap: `valid_up_to` marks the valid prefix.
                    self.count_chars(std::str::from_utf8(&data[..valid]).unwrap());
                    match e.error_len() {
                        // Invalid bytes decode to U+FFFD, which is not whitespace.
                        Some(n) => {
                            self.char_seen(false);
                            data = &data[valid + n..];
                        }
                        // Sequence cut off by the chunk boundary: wait for more.
                        None => {
                            self.pending = data[valid..].to_vec();
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Call once the body has ended. A sequence still incomplete at that
    /// point decodes to U+FFFD.
    fn finish(&mut self) {
        if !std::mem::take(&mut self.pending).is_empty() {
            self.char_seen(false);
        }
    }

    fn lines(&self) -> u64 {
        self.newlines + self.trailing as u64
    }

    fn count_chars(&mut self, s: &str) {
        for c in s.chars() {
            self.char_seen(c.is_whitespace());
        }
    }

    fn char_seen(&mut self, whitespace: bool) {
        if whitespace {
            self.in_word = false;
        } else if !self.in_word {
            self.in_word = true;
            self.words += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{describe_error, run_fuzz, BodyStats, FuzzHandle, FuzzProgress};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// Answers every request with a small 200, holding back any path that
    /// contains "slow" for 800ms. Returns the server's base URL.
    fn spawn_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 1024];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if String::from_utf8_lossy(&buf[..n]).contains("slow") {
                        std::thread::sleep(Duration::from_millis(800));
                    }
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
                    );
                });
            }
        });
        format!("http://{addr}")
    }

    /// Runs a scan and returns each batch's size and arrival time, plus every
    /// progress update.
    async fn scan(url: &str, words: Vec<String>) -> (Vec<(usize, Duration)>, Vec<FuzzProgress>) {
        let config = serde_json::from_value(serde_json::json!({
            "url": format!("{url}/FUZZ"),
            "method": "GET",
            "headers": {},
            "body": null,
            "concurrency": 20,
            "timeout_ms": 5000,
            "follow_redirects": false,
            "delay_ms": 0,
        }))
        .unwrap();
        let filters = serde_json::from_value(serde_json::json!({
            "exclude_status": [],
            "include_status": [],
            "exclude_size": [],
            "exclude_words": [],
            "exclude_lines": [],
        }))
        .unwrap();

        let batches = Arc::new(Mutex::new(Vec::new()));
        let progress = Arc::new(Mutex::new(Vec::new()));
        let start = Instant::now();
        run_fuzz(
            config,
            filters,
            words,
            FuzzHandle::new(),
            {
                let batches = batches.clone();
                move |results: Vec<_>| batches.lock().unwrap().push((results.len(), start.elapsed()))
            },
            {
                let progress = progress.clone();
                move |p| progress.lock().unwrap().push(p)
            },
        )
        .await
        .unwrap();
        let batches = batches.lock().unwrap().clone();
        let progress = std::mem::take(&mut *progress.lock().unwrap());
        (batches, progress)
    }

    #[tokio::test]
    async fn fast_scan_delivers_every_result_in_few_batches() {
        let url = spawn_server();
        let words: Vec<String> = (0..500).map(|i| format!("w{i}")).collect();
        let (batches, progress) = scan(&url, words).await;

        assert_eq!(batches.iter().map(|b| b.0).sum::<usize>(), 500);
        assert!(batches.len() < 100, "{} batches", batches.len());
        assert!(progress.len() < 100, "{} progress updates", progress.len());
        let last = progress.last().unwrap();
        assert_eq!((last.done, last.total, last.matches), (500, 500, 500));
    }

    #[tokio::test]
    async fn results_are_not_held_back_by_a_slow_request() {
        let url = spawn_server();
        let (batches, _) = scan(&url, vec!["fast".into(), "slow".into()]).await;

        // The fast result must arrive on a timer tick, not when "slow" finishes.
        assert_eq!(batches.len(), 2, "{batches:?}");
        assert!(batches[0].1 < Duration::from_millis(500), "{batches:?}");
    }

    #[tokio::test]
    async fn refused_connection_names_the_cause() {
        // Port 1 on loopback: nothing listens there, so the connect is refused.
        let err = reqwest::get("http://127.0.0.1:1/").await.unwrap_err();
        let msg = describe_error(&err, 5000);
        println!("{msg}");
        assert!(msg.starts_with("Connection failed: "), "{msg}");
    }

    fn expected(body: &[u8]) -> (u64, u64, u64) {
        let text = String::from_utf8_lossy(body);
        (
            body.len() as u64,
            text.split_whitespace().count() as u64,
            text.lines().count() as u64,
        )
    }

    fn streamed(body: &[u8], chunk_size: usize) -> (u64, u64, u64) {
        let mut stats = BodyStats::default();
        for chunk in body.chunks(chunk_size) {
            stats.feed(chunk);
        }
        stats.finish();
        (stats.size, stats.words, stats.lines())
    }

    #[test]
    fn matches_whole_body_counting_at_every_chunk_size() {
        let bodies: &[&[u8]] = &[
            b"",
            b"\n",
            b"one",
            b"one two\nthree\n",
            b"a\r\nb\r\n\r\nc",
            "שלום עולם\u{00A0}x\u{3000}y\n".as_bytes(),
            b"bad \xff\xfe utf8 \xe2\x82 end",
            b"cut off at the end \xe2\x82",
            b"  leading and trailing  \n\n",
        ];
        for body in bodies {
            for chunk_size in 1..=body.len().max(1) {
                assert_eq!(
                    streamed(body, chunk_size),
                    expected(body),
                    "body {body:?}, chunk size {chunk_size}"
                );
            }
        }
    }
}
