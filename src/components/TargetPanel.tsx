import { BODY_METHODS, type FuzzConfig, type HttpMethod } from "../types";

const METHODS: HttpMethod[] = ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH"];

export function TargetPanel({
  config,
  onChange,
  disabled,
}: {
  config: FuzzConfig;
  onChange: (c: FuzzConfig) => void;
  disabled: boolean;
}) {
  return (
    <section className="panel">
      <h2 className="panel__title">יעד</h2>

      <label className="field">
        <span>כתובת — FUZZ מסמן את נקודת ההחלפה</span>
        <input
          className="mono"
          value={config.url}
          disabled={disabled}
          onChange={(e) => onChange({ ...config, url: e.target.value })}
          placeholder="https://target.example/FUZZ"
        />
      </label>

      <label className="field">
        <span>שיטה</span>
        <select
          value={config.method}
          disabled={disabled}
          onChange={(e) => onChange({ ...config, method: e.target.value as HttpMethod })}
        >
          {METHODS.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </label>

      {BODY_METHODS.includes(config.method) && (
        <label className="field">
          <span>Body — אפשר FUZZ גם כאן</span>
          <textarea
            className="mono"
            value={config.body ?? ""}
            disabled={disabled}
            onChange={(e) => onChange({ ...config, body: e.target.value })}
            rows={3}
          />
        </label>
      )}

      <div className="field-row">
        <label className="field">
          <span>בו-זמניות</span>
          <input
            type="number"
            min={1}
            max={500}
            value={config.concurrency}
            disabled={disabled}
            onChange={(e) => onChange({ ...config, concurrency: Number(e.target.value) })}
          />
        </label>
        <label className="field">
          <span>Timeout (ms)</span>
          <input
            type="number"
            min={100}
            value={config.timeout_ms}
            disabled={disabled}
            onChange={(e) => onChange({ ...config, timeout_ms: Number(e.target.value) })}
          />
        </label>
      </div>

      <label className="field field--row">
        <input
          type="checkbox"
          checked={config.follow_redirects}
          disabled={disabled}
          onChange={(e) => onChange({ ...config, follow_redirects: e.target.checked })}
        />
        <span>עקוב אחרי הפניות (redirects)</span>
      </label>
    </section>
  );
}
