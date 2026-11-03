import { HttpStatusError, InputError, NetworkError } from "./errors.ts";

export interface HttpOptions {
  /** Injected for tests and non-browser hosts. Defaults to the global `fetch`. */
  fetch?: typeof fetch;
  /** Per-request budget. A request that exceeds it fails as a `NetworkError`. */
  timeoutMs?: number;
}

export const DEFAULT_TIMEOUT_MS = 10_000;
const MAX_RESPONSE_CHARS = 8 * 1024 * 1024;
const MAX_ERROR_BODY_CHARS = 300;

export function parseHttpUrl(value: string, label: string): URL {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new InputError(`${label} is not a URL`);
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new InputError(`${label} must be http or https`);
  }
  return url;
}

/**
 * One request, one bounded text body. Transport failures become `NetworkError`; a non-2xx answer
 * becomes `HttpStatusError`. The caller never sees a partially read body.
 */
export async function requestText(
  url: URL,
  init: RequestInit,
  options: HttpOptions,
  label: string,
): Promise<{ status: number; text: string }> {
  const impl = options.fetch ?? globalThis.fetch;
  let response: Response;
  let text: string;
  try {
    response = await impl(url, {
      ...init,
      credentials: "omit",
      signal: AbortSignal.timeout(options.timeoutMs ?? DEFAULT_TIMEOUT_MS),
    });
    text = await response.text();
  } catch (cause) {
    throw new NetworkError(`${label} is unreachable`, { cause });
  }
  if (text.length > MAX_RESPONSE_CHARS)
    throw new NetworkError(`${label} sent an oversized response`);
  if (!response.ok)
    throw new HttpStatusError(response.status, text.slice(0, MAX_ERROR_BODY_CHARS).trim());
  return { status: response.status, text };
}
