use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

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
/// `on_result` / `on_progress` are callbacks rather than a return value —
/// a Tauri command uses them to emit events to the frontend as each
/// response comes back, so the results table fills in live instead of the
/// whole run having to finish (and sit buffered in memory) before the user
/// sees a single row. That matters once a wordlist runs into the tens of
/// thousands of entries.
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
    mut on_result: FR,
    mut on_progress: FP,
) -> Result<(), String>
where
    FR: FnMut(FuzzResult) + Send,
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
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            // Raw bytes, not `text()`: decoding would swap invalid
                            // UTF-8 for U+FFFD and skew the size the filters key on.
                            let body = resp.bytes().await.unwrap_or_default();
                            let body_text = String::from_utf8_lossy(&body);
                            FuzzResult {
                                word: word.clone(),
                                url: built.url.clone(),
                                status,
                                size: body.len() as u64,
                                words: body_text.split_whitespace().count() as u64,
                                lines: body_text.lines().count() as u64,
                                duration_ms: req_start.elapsed().as_millis() as u64,
                                error: None,
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
                            error: Some(e.to_string()),
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
    while let Some(result) = rx.recv().await {
        done += 1;
        if filters.matches(&result) {
            matches += 1;
            on_result(result);
        }
        let elapsed = start.elapsed().as_secs_f64().max(0.001);
        on_progress(FuzzProgress {
            done,
            total,
            matches,
            rps: done as f64 / elapsed,
        });
    }

    Ok(())
}
