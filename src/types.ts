export type HttpMethod =
  | "GET"
  | "POST"
  | "PUT"
  | "DELETE"
  | "HEAD"
  | "OPTIONS"
  | "PATCH";

/** Methods the UI offers a body field for; any other method is sent without one. */
export const BODY_METHODS: HttpMethod[] = ["POST", "PUT", "PATCH"];

export interface FuzzConfig {
  url: string;
  method: HttpMethod;
  headers: Record<string, string>;
  body: string | null;
  concurrency: number;
  timeout_ms: number;
  follow_redirects: boolean;
  delay_ms: number;
}

export interface FilterConfig {
  exclude_status: number[];
  include_status: number[];
  exclude_size: number[];
  exclude_words: number[];
  exclude_lines: number[];
}

export interface FuzzResult {
  word: string;
  url: string;
  status: number;
  size: number;
  words: number;
  lines: number;
  duration_ms: number;
  error: string | null;
}

export interface FuzzProgress {
  done: number;
  total: number;
  matches: number;
  rps: number;
}

// Matches the Rust enum's #[serde(tag = "kind", content = "value")] shape.
export type WordlistSource =
  | { kind: "Bundled"; value: string }
  | { kind: "File"; value: string }
  | { kind: "Inline"; value: string[] };
