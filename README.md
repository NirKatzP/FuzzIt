# FuzzIt

A lightweight Windows GUI fuzzer inspired by [ffuf](https://github.com/ffuf/ffuf) —
with its own self-contained fuzzing engine (no dependency on the `ffuf.exe`
binary), a small footprint, and wordlists you load into the app.

## Why this stack (and not something else)

**Tauri (Rust backend + React/TS webview) instead of Electron, WinUI/WPF, or
a Python GUI.**
Tauri renders through the OS's own WebView2 on Windows instead of shipping a
bundled Chromium like Electron does, so the installed app is tens of MB
instead of hundreds. The backend is a real native Rust binary, not an
interpreted runtime, so the fuzzing engine itself runs at native speed. You
still get a modern, easily-styled UI via web tech, which is the "innovative
but light" combination the brief asked for — C#/WinUI gets you native
Windows chrome but a slower, more verbose path to a distinctive UI; a Python
GUI (PySide6) is the fastest to prototype but produces the heaviest
single-file executable of the three.

**The fuzzing logic is implemented in the app, not shelled out to ffuf.exe.**
This was the one hard requirement: the app owns the HTTP loop, concurrency,
and filtering itself (`src-tauri/src/fuzzer/`). That means one less external
binary to bundle, trust, or keep in sync, and the async engine can be built
to match exactly how the UI wants to consume results — streamed live,
rather than parsed back out of a subprocess's stdout.

**One substitution mechanism covers directory, parameter, and vhost
fuzzing.** `fuzzer::request::build_request` replaces the `FUZZ` keyword
everywhere it appears — URL, header values, body — in a single pass. Put
`FUZZ` in the URL path and you get directory/file fuzzing; put it in the
`Host` header and you get vhost fuzzing; put it in the body or a header
value and you get parameter fuzzing. It's the same mechanism ffuf itself
uses, so there's no separate "mode" to pick in the UI — just where you type
`FUZZ`.

**Concurrency: one Tokio task per word, gated by a `Semaphore`.** This is
the direct equivalent of ffuf's `-t` flag. Results come back over a bounded
`mpsc` channel that's drained concurrently with the producer (not after
it), so the engine never buffers a whole run in memory — it can handle
wordlists far larger than what's comfortable to hold as one `Vec` of
results, and the UI's results table fills in live, request by request.

**reqwest with `rustls-tls`, not the platform TLS stack.** Avoids requiring
OpenSSL headers/libraries to build on Windows, which is one of the more
common sources of "works on my machine" build failures for Rust networking
projects. Keeps the build self-contained.

**Two small wordlists are compiled into the binary; everything else is
loaded from disk at runtime.** `include_str!` embeds `common-small.txt`
(~150 words) and `api-small.txt` (~95 words) directly, so the app is usable
the moment it's installed, with zero setup. Larger or custom lists — which
was explicitly part of the brief — are read from disk via a native file
picker (`tauri-plugin-dialog`) instead, which is what keeps the compiled
binary itself small.

**UI language is Hebrew; layout direction is LTR, not mirrored RTL.** The
content this tool is built around — URLs, headers, hex, status codes,
response sizes — is inherently left-to-right technical data. Mirroring the
whole shell into RTL would hurt scanability of the results table precisely
where speed of scanning matters most. This mirrors how most Hebrew-market
developer tooling behaves: Hebrew labels, LTR data and layout.

**Visual design.** Dark navy-ink base (`#0E1621`) with a warm copper accent
(`#E8954A`), rather than either of the two generic looks AI-generated tools
tend to default to (near-black with a neon-green "hacker" accent, or warm
cream with a terracotta accent). Status codes are color-coded because that
color is *information* (response class), not decoration — scanning a
results table by color is exactly what a fuzzer's output is for.
Typography pairs **Space Grotesk** (UI chrome, headings) with **JetBrains
Mono** (the results table, URLs, status codes) — the monospace choice is
functional, not aesthetic: tabular figures keep columns of sizes and status
codes aligned and scannable, and avoid 0/O and 1/l ambiguity in paths.

## Project structure

```
FuzzIt/
├── src-tauri/                  Rust backend
│   ├── capabilities/           Tauri v2 permissions (events, file dialog)
│   ├── icons/                  App icons (placeholder — regenerate, see below)
│   ├── src/
│   │   ├── main.rs             App entry point, Tauri wiring
│   │   ├── commands.rs         start_fuzz / stop_fuzz / list_bundled_wordlists
│   │   ├── fuzzer/
│   │   │   ├── engine.rs       The async fuzz loop (concurrency, streaming)
│   │   │   ├── request.rs      FUZZ-keyword substitution
│   │   │   ├── filters.rs      Status/size/words/lines include-exclude logic
│   │   │   └── wordlist.rs     Bundled + on-disk wordlist loading
│   │   └── wordlists/          The two bundled starter lists
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                        React frontend
│   ├── App.tsx                 State + wiring to Tauri commands/events
│   ├── components/              TargetPanel, WordlistPanel, FiltersPanel,
│   │                             ResultsTable, RunControls, StatusBadge
│   ├── styles/globals.css      Design tokens (color, type, layout)
│   └── types.ts                 TS mirrors of the Rust structs
└── package.json
```

## Building it

You'll need, on the Windows machine you build on:

1. **Rust** (stable) — https://rustup.rs
2. **Node.js** 18+ and npm
3. **Tauri's Windows prerequisites** — the WebView2 runtime (pre-installed
   on current Windows 10/11) and the "Desktop development with C++"
   workload from the Visual Studio Build Tools. Full list:
   https://v2.tauri.app/start/prerequisites/

Then, from the project root:

```sh
npm install
npm run tauri dev       # run it in development, with hot reload
npm run tauri build     # produce a release .msi / NSIS installer
```

`src-tauri/icons/` only holds a generated placeholder (Windows builds,
including `tauri dev`, fail without an `icon.ico`). Replace it with a real
icon set before a release build:

```sh
npm run tauri icon path/to/a-1024x1024-source-image.png
```

### Fonts

The two typefaces referenced in `globals.css` aren't bundled in this
scaffold (no network access was available while generating it). Both are
open-source (SIL OFL) — download the variable `.woff2` builds of
[Space Grotesk](https://fonts.google.com/specimen/Space+Grotesk) and
[JetBrains Mono](https://www.jetbrains.com/lp/mono/) and drop them into
`src/assets/fonts/` using the filenames already referenced in
`globals.css`. Until then the app falls back to Segoe UI / Consolas, so
it's fully usable either way.

## Known MVP limits / natural next steps

- **Export.** The results table has no Export button wired up yet —
  straightforward to add (serialize `FuzzResult[]` to CSV/JSON client-side).
- **Very large wordlists.** `wordlist::load_from_file` reads the whole file
  into memory. Fine up into the low millions of lines; a streaming reader
  would be the next step for anything bigger.
- **No response preview.** Clicking a result row doesn't yet show the full
  response body/headers — useful for triaging hits, not implemented yet.
- **Stop vs. pause.** "Stop" cancels scheduling of new requests but lets
  in-flight ones finish naturally; there's no pause/resume.
- **Single theme.** Only the dark palette above exists; no light-mode
  toggle yet.
