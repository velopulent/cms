/**
 * Parse a JSON string without throwing. Returns `fallback` on any parse error
 * (malformed/corrupted API payloads) so a single bad value can't crash a route.
 * If `value` is already a non-string (e.g. the API returned a parsed object),
 * it is returned as-is.
 */
export function safeJsonParse<T>(value: unknown, fallback: T): T {
  if (typeof value !== "string") {
    return (value as T) ?? fallback;
  }
  try {
    return JSON.parse(value) as T;
  } catch {
    return fallback;
  }
}
