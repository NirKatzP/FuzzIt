import { useEffect, useRef, useState } from "react";
import type { FuzzResult } from "../types";
import { StatusBadge } from "./StatusBadge";

// Must match the row height in globals.css: only the rows on screen are in the
// DOM, and the spacers that stand in for the rest are sized from this.
const ROW_HEIGHT = 32;
const OVERSCAN = 10;
const COLUMN_COUNT = 7;

function Spacer({ height }: { height: number }) {
  if (height <= 0) return null;
  return (
    <tr aria-hidden="true">
      <td className="results-table__spacer" colSpan={COLUMN_COUNT} style={{ height }} />
    </tr>
  );
}

export function ResultsTable({
  results,
  count,
  totalMatches,
}: {
  // Appended to in place by the parent; `count` is what signals a change.
  results: FuzzResult[];
  count: number;
  totalMatches: number;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const observer = new ResizeObserver(() => setViewportHeight(wrap.clientHeight));
    observer.observe(wrap);
    return () => observer.disconnect();
  }, []);

  const visibleRows = Math.ceil(viewportHeight / ROW_HEIGHT);
  // Clamped because `scrollTop` can be left over from a longer previous run.
  const firstVisible = Math.min(Math.floor(scrollTop / ROW_HEIGHT), Math.max(0, count - visibleRows));
  const start = Math.max(0, firstVisible - OVERSCAN);
  const end = Math.min(count, firstVisible + visibleRows + OVERSCAN);

  return (
    <>
      {totalMatches > count && count > 0 && (
        <div className="results-notice" role="status" dir="rtl">
          מוצגות {count.toLocaleString()} התוצאות הראשונות מתוך {totalMatches.toLocaleString()}. כדאי להוסיף מסננים.
        </div>
      )}
      <div className="results-table-wrap" ref={wrapRef} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
        {count === 0 ? (
          <div className="empty-state">אין עדיין תוצאות — הפעילו סריקה כדי להתחיל.</div>
        ) : (
          <table className="results-table">
            <colgroup>
              <col style={{ width: 80 }} />
              <col />
              <col style={{ width: 90 }} />
              <col style={{ width: 80 }} />
              <col style={{ width: 80 }} />
              <col style={{ width: 80 }} />
              <col />
            </colgroup>
            <thead>
              <tr>
                <th>סטטוס</th>
                <th>נתיב</th>
                <th>גודל</th>
                <th>מילים</th>
                <th>שורות</th>
                <th>זמן</th>
                <th>פרטים</th>
              </tr>
            </thead>
            <tbody>
              <Spacer height={start * ROW_HEIGHT} />
              {results.slice(start, end).map((r, i) => (
                <tr key={start + i}>
                  <td>
                    <StatusBadge status={r.status} />
                  </td>
                  <td className="mono" title={r.word}>
                    {r.word}
                  </td>
                  <td className="mono">{r.size.toLocaleString()}</td>
                  <td className="mono">{r.words.toLocaleString()}</td>
                  <td className="mono">{r.lines.toLocaleString()}</td>
                  <td className="mono">{r.duration_ms}ms</td>
                  <td title={r.error ? `${r.url}\n${r.error}` : r.url}>
                    {r.error && <span className="result-details__error">{r.error}</span>}
                  </td>
                </tr>
              ))}
              <Spacer height={(count - end) * ROW_HEIGHT} />
            </tbody>
          </table>
        )}
      </div>
    </>
  );
}
