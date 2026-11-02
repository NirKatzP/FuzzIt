function statusClass(status: number): string {
  if (status === 0) return "status-badge status-badge--error";
  if (status < 300) return "status-badge status-badge--ok";
  if (status < 400) return "status-badge status-badge--redirect";
  if (status < 500) return "status-badge status-badge--client";
  return "status-badge status-badge--server";
}

export function StatusBadge({ status }: { status: number }) {
  return <span className={statusClass(status)}>{status === 0 ? "ERR" : status}</span>;
}
