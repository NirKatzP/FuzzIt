import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import { TargetPanel } from "./components/TargetPanel";
import { WordlistPanel } from "./components/WordlistPanel";
import { FiltersPanel } from "./components/FiltersPanel";
import { ResultsTable } from "./components/ResultsTable";
import { RunControls } from "./components/RunControls";
import {
  BODY_METHODS,
  type FilterConfig,
  type FuzzConfig,
  type FuzzProgress,
  type FuzzResult,
  type WordlistSource,
} from "./types";

// Past this many rows the filters clearly aren't separating anything, and
// holding more would only eat memory on a long scan. The scan itself goes on.
const MAX_RESULTS = 100_000;

const DEFAULT_CONFIG: FuzzConfig = {
  url: "https://example.com/FUZZ",
  method: "GET",
  headers: {},
  body: null,
  concurrency: 40,
  timeout_ms: 7000,
  follow_redirects: false,
  delay_ms: 0,
};

const DEFAULT_FILTERS: FilterConfig = {
  exclude_status: [404],
  include_status: [],
  exclude_size: [],
  exclude_words: [],
  exclude_lines: [],
};

function clampInt(n: number, min: number, max: number, fallback: number): number {
  return Number.isFinite(n) ? Math.min(max, Math.max(min, Math.round(n))) : fallback;
}

// Number inputs hand back 0 for an empty field and NaN for junk; NaN
// serializes as null and fails deserialization on the Rust side, and a 0 ms
// timeout fails every request. Normalize before sending. Same for the body:
// switching POST -> GET hides the field but leaves the old value in state.
function toRunnableConfig(c: FuzzConfig): FuzzConfig {
  return {
    ...c,
    body: BODY_METHODS.includes(c.method) && c.body ? c.body : null,
    concurrency: clampInt(c.concurrency, 1, 500, DEFAULT_CONFIG.concurrency),
    timeout_ms: c.timeout_ms > 0 ? clampInt(c.timeout_ms, 100, 600_000, DEFAULT_CONFIG.timeout_ms) : DEFAULT_CONFIG.timeout_ms,
    delay_ms: clampInt(c.delay_ms, 0, 600_000, 0),
  };
}

export default function App() {
  const [config, setConfig] = useState<FuzzConfig>(DEFAULT_CONFIG);
  const [filters, setFilters] = useState<FilterConfig>(DEFAULT_FILTERS);
  const [wordlistSource, setWordlistSource] = useState<WordlistSource>({
    kind: "Bundled",
    value: "common-small",
  });
  const [resultCount, setResultCount] = useState(0);
  const [progress, setProgress] = useState<FuzzProgress | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Results stream in as batches. They're appended in place, since copying
  // the whole array per batch gets expensive on a long scan; `resultCount` is
  // the state update that tells React there's more to render.
  const resultsRef = useRef<FuzzResult[]>([]);

  useEffect(() => {
    const unlistenResults = listen<FuzzResult[]>("fuzz://results", (e) => {
      const results = resultsRef.current;
      const room = MAX_RESULTS - results.length;
      for (const result of e.payload.slice(0, room)) {
        results.push(result);
      }
      setResultCount(results.length);
    });
    const unlistenProgress = listen<FuzzProgress>("fuzz://progress", (e) => {
      setProgress(e.payload);
    });
    const unlistenComplete = listen("fuzz://complete", () => {
      setRunning(false);
    });
    return () => {
      unlistenResults.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenComplete.then((f) => f());
    };
  }, []);

  async function handleStart() {
    resultsRef.current = [];
    setResultCount(0);
    setProgress(null);
    setError(null);
    setRunning(true);
    try {
      // Resolves only once the run is over, so this (not the
      // fuzz://complete event alone) is what reliably re-enables Start.
      await invoke("start_fuzz", {
        args: { config: toRunnableConfig(config), filters, wordlist_source: wordlistSource },
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }

  async function handleStop() {
    try {
      await invoke("stop_fuzz");
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleImportWordlist() {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "Wordlist", extensions: ["txt"] }],
      });
      if (typeof path === "string") {
        setWordlistSource({ kind: "File", value: path });
      }
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="app-shell">
      <header className="app-titlebar">
        <span className="app-titlebar__dot" />
        <h1>FuzzIt</h1>
      </header>
      <div className="app-body">
        <aside className="app-sidebar">
          <TargetPanel config={config} onChange={setConfig} disabled={running} />
          <WordlistPanel
            source={wordlistSource}
            onSelectBundled={(id) => setWordlistSource({ kind: "Bundled", value: id })}
            onImport={handleImportWordlist}
            disabled={running}
          />
          <FiltersPanel filters={filters} onChange={setFilters} disabled={running} />
        </aside>
        <main className="app-main">
          <RunControls running={running} progress={progress} error={error} onStart={handleStart} onStop={handleStop} />
          <ResultsTable results={resultsRef.current} count={resultCount} totalMatches={progress?.matches ?? 0} />
        </main>
      </div>
    </div>
  );
}
