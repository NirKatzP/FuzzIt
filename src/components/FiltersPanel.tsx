import type { FilterConfig } from "../types";

function parseList(s: string): number[] {
  return s
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean)
    .map(Number)
    .filter((n) => !Number.isNaN(n));
}

export function FiltersPanel({
  filters,
  onChange,
  disabled,
}: {
  filters: FilterConfig;
  onChange: (f: FilterConfig) => void;
  disabled: boolean;
}) {
  return (
    <section className="panel">
      <h2 className="panel__title">סינון</h2>
      <label className="field">
        <span>הסתר קודי סטטוס</span>
        <input
          className="mono"
          value={filters.exclude_status.join(", ")}
          disabled={disabled}
          onChange={(e) => onChange({ ...filters, exclude_status: parseList(e.target.value) })}
          placeholder="404, 400"
        />
      </label>
      <label className="field">
        <span>הצג רק קודי סטטוס (ריק = הכל)</span>
        <input
          className="mono"
          value={filters.include_status.join(", ")}
          disabled={disabled}
          onChange={(e) => onChange({ ...filters, include_status: parseList(e.target.value) })}
          placeholder="200, 301, 403"
        />
      </label>
    </section>
  );
}
