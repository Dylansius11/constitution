import { shortValue, spokenValue, utcStamp } from "../../lib/display.ts";
import type { Source } from "../../lib/state.ts";

export function lastObserved<T>(source: Source<T>): { value: T; observedAtMs: number } | undefined {
  return source.status === "ok"
    ? source
    : source.status === "unavailable"
      ? source.last
      : undefined;
}

export async function observe<T>(previous: Source<T>, read: () => Promise<T>): Promise<Source<T>> {
  try {
    return { status: "ok", value: await read(), observedAtMs: Date.now() };
  } catch (error) {
    const last = lastObserved(previous);
    return {
      status: "unavailable",
      failures: previous.status === "unavailable" ? previous.failures + 1 : 1,
      error: error instanceof Error ? error.message : "Read failed",
      ...(last === undefined ? {} : { last }),
    };
  }
}

/** One in-flight read, no hidden-tab polling. The caller owns the cadence and error policy. */
export function pollVisible(read: () => Promise<void>, intervalMs = 15_000): () => void {
  let stopped = false;
  let busy = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = async (): Promise<void> => {
    if (stopped || document.hidden || busy) return;
    busy = true;
    try {
      await read();
    } finally {
      busy = false;
      if (!stopped && !document.hidden)
        timer = setTimeout(() => {
          void tick();
        }, intervalMs);
    }
  };
  const visibility = (): void => {
    clearTimeout(timer);
    if (!document.hidden) void tick();
  };
  document.addEventListener("visibilitychange", visibility);
  void tick();
  return () => {
    stopped = true;
    clearTimeout(timer);
    document.removeEventListener("visibilitychange", visibility);
  };
}

export function text(root: ParentNode, selector: string, value: string): void {
  const target = root.querySelector(selector);
  if (target) target.textContent = value;
}

export function timestamp(ms: number, prefix = "Observed"): HTMLTimeElement {
  const time = document.createElement("time");
  time.dateTime = new Date(ms).toISOString();
  time.textContent = `${prefix} ${utcStamp(ms)}`;
  return time;
}

/** Mirrors HashText.astro; copying is handled by CopyButton's delegated page handler. */
export function hashValue(
  value: string,
  label: string,
  kind: "hash" | "address" | "near" = "hash",
): HTMLElement {
  const narrow = shortValue(value, kind, "narrow");
  const wide = shortValue(value, kind, "wide");
  const wrap = document.createElement("span");
  wrap.className = "hash hash--light";

  const details = document.createElement("details");
  details.className = "hash__details";
  const summary = document.createElement("summary");
  summary.setAttribute("aria-label", `${spokenValue(label, value)}. Show full value.`);

  const short = document.createElement("code");
  short.className = "hash__short";
  short.setAttribute("aria-hidden", "true");
  const narrowValue = document.createElement("span");
  narrowValue.className = "hash__narrow";
  narrowValue.textContent = narrow;
  const wideValue = document.createElement("span");
  wideValue.className = "hash__wide";
  wideValue.textContent = wide;
  short.append(narrowValue, wideValue);
  summary.append(short);

  if (narrow !== value || wide !== value) {
    const more = document.createElement("span");
    more.className = "hash__more";
    more.setAttribute("aria-hidden", "true");
    const closed = document.createElement("span");
    closed.className = "hash__closed";
    closed.textContent = "Full";
    const opened = document.createElement("span");
    opened.className = "hash__opened";
    opened.textContent = "Less";
    more.append(closed, opened);
    summary.append(more);
  }

  const full = document.createElement("code");
  full.className = "hash__full";
  full.textContent = value;
  details.append(summary, full);

  const copyLabel = label.toLowerCase();
  const copy = document.createElement("button");
  copy.type = "button";
  copy.className = "copy copy--light";
  copy.dataset["copyValue"] = value;
  copy.dataset["copyLabel"] = copyLabel;
  copy.setAttribute("aria-label", `Copy ${copyLabel}`);

  const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  icon.setAttribute("viewBox", "0 0 16 16");
  icon.setAttribute("width", "14");
  icon.setAttribute("height", "14");
  icon.setAttribute("aria-hidden", "true");
  icon.setAttribute("focusable", "false");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "5.5");
  rect.setAttribute("y", "5.5");
  rect.setAttribute("width", "8");
  rect.setAttribute("height", "8");
  rect.setAttribute("rx", "1.5");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute(
    "d",
    "M10.5 3.5v-.5A1.5 1.5 0 0 0 9 1.5H4A1.5 1.5 0 0 0 2.5 3v5A1.5 1.5 0 0 0 4 9.5h.5",
  );
  icon.append(rect, path);
  const face = document.createElement("span");
  face.className = "copy__text";
  face.setAttribute("aria-hidden", "true");
  face.textContent = "Copy";
  copy.append(icon, face);

  wrap.append(details, copy);
  return wrap;
}

export function replaceValue(
  target: HTMLElement,
  value: string,
  label: string,
  kind: "hash" | "address" | "near" = "hash",
): void {
  if (target.dataset["value"] === value) return;
  target.dataset["value"] = value;
  target.replaceChildren(hashValue(value, label, kind));
}
