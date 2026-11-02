import type { FuzzResult } from "../types";
import { StatusBadge } from "./StatusBadge";

export function ResultsTable({ results }: { results: FuzzResult[] }) {
  if (results.length === 0) {
    return (
      <div className="results-table-wrap">
        <div className="empty-state">אין עדיין תוצאות — הפעילו סריקה כדי להתחיל.</div>
      </div>
    );
  }

  return (
    <div className="results-table-wrap">
      <table className="results-table">
        <thead>
          <tr>
            <th>סטטוס</th>
            <th>נתיב</th>
            <th>גודל</th>
            <th>מילים</th>
            <th>שורות</th>
            <th>זמן</th>
          </tr>
        </thead>
        <tbody>
          {results.map((r, i) => (
            <tr key={i}>
              <td>
                <StatusBadge status={r.status} />
              </td>
              <td className="mono">{r.word}</td>
              <td className="mono">{r.size.toLocaleString()}</td>
              <td className="mono">{r.words.toLocaleString()}</td>
              <td className="mono">{r.lines.toLocaleString()}</td>
              <td className="mono">{r.duration_ms}ms</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
