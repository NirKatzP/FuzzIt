import type { FuzzProgress } from "../types";

export function RunControls({
  running,
  progress,
  error,
  onStart,
  onStop,
}: {
  running: boolean;
  progress: FuzzProgress | null;
  error: string | null;
  onStart: () => void;
  onStop: () => void;
}) {
  const pct = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;

  return (
    <div className="run-controls">
      <button className="btn btn--primary" onClick={running ? onStop : onStart}>
        {running ? "עצור" : "התחל סריקה"}
      </button>
      <div className="run-progress">
        <div className="run-progress__bar">
          <div className="run-progress__fill" style={{ width: `${pct}%` }} />
        </div>
        <div className="run-progress__stats mono">
          {progress
            ? `${progress.done}/${progress.total} · ${progress.matches} תוצאות · ${progress.rps.toFixed(1)} req/s`
            : "ממתין להפעלה"}
        </div>
        {error && (
          <div className="run-progress__error" role="alert">
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
