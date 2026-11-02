import type { WordlistSource } from "../types";

const BUNDLED: { id: string; label: string }[] = [
  { id: "common-small", label: "נתיבים נפוצים (~150 מילים)" },
  { id: "api-small", label: "נקודות קצה API (~95 מילים)" },
];

export function WordlistPanel({
  source,
  onSelectBundled,
  onImport,
  disabled,
}: {
  source: WordlistSource;
  onSelectBundled: (id: string) => void;
  onImport: () => void;
  disabled: boolean;
}) {
  return (
    <section className="panel">
      <h2 className="panel__title">Wordlist</h2>
      <div className="wordlist-options">
        {BUNDLED.map((w) => (
          <label key={w.id} className="field field--row">
            <input
              type="radio"
              name="wordlist"
              checked={source.kind === "Bundled" && source.value === w.id}
              disabled={disabled}
              onChange={() => onSelectBundled(w.id)}
            />
            <span>{w.label}</span>
          </label>
        ))}
        <label className="field field--row">
          <input
            type="radio"
            name="wordlist"
            checked={source.kind === "File"}
            onClick={onImport}
            readOnly
            disabled={disabled}
          />
          <span>
            קובץ מהמחשב
            {source.kind === "File" ? ` — ${source.value.split(/[\\/]/).pop()}` : ""}
          </span>
        </label>
      </div>
      <button className="btn btn--secondary" onClick={onImport} disabled={disabled}>
        ייבוא קובץ wordlist…
      </button>
    </section>
  );
}
