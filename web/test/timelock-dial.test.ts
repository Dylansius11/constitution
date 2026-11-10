import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../src/components/motion/gsap.ts", () => ({
  MOTION_OK: "(prefers-reduced-motion: no-preference)",
  gsap: { matchMedia: () => ({ add: (_query: string, setup: () => void) => setup() }) },
  ScrollTrigger: { create: () => undefined },
}));

import { initTimelock } from "../src/components/motion/timelock.ts";

// Minimal DOM doubles keep this node-environment test independent of a browser dependency.
class Element {
  textContent = "";
  dataset: Record<string, string> = {};
  style = { transform: "", removeProperty: () => undefined };
  classList = { toggle: () => undefined, remove: () => undefined };
  children: Element[] = [];
  attributes = new Map<string, string>();
  setAttribute(name: string, value: string): void { this.attributes.set(name, value); }
  getAttribute(name: string): string | null { return this.attributes.get(name) ?? null; }
  append(...nodes: Element[]): void { this.children.push(...nodes); }
  replaceChildren(...nodes: Element[]): void { this.children = nodes; }
  cloneNode(): Element { return new Element(); }
  querySelector<T>(selector: string): T | null {
    return (nodes.get(selector) ?? null) as T | null;
  }
  querySelectorAll<T>(selector: string): T[] {
    return (selector === ".dial__seg" ? (nodes.get(".dial__segments")?.children ?? []) : []) as T[];
  }
}
const nodes = new Map<string, Element>();
const doc = new EventTarget() as EventTarget & {
  querySelector: (selector: string) => Element | null;
  createElementNS: (_namespace: string, _name: string) => Element;
};
doc.querySelector = (selector) => nodes.get(selector) ?? null;
doc.createElementNS = () => new Element();

afterEach(() => {
  nodes.clear();
  vi.unstubAllGlobals();
});

describe("timelock dial", () => {
  it.each([1, 30])("uses an observed %i-day gate delay rather than the default illustration", (days) => {
    for (const selector of ["#timelock", ".timelock__grid", ".dial__hand", ".dial__count", ".dial__big", ".dial__small", ".dial__description", ".dial__segments", ".dial__days", ".dial__seg", ".dial__day"]) {
      nodes.set(selector, new Element());
    }
    nodes.get(".dial__big")!.textContent = "3 days";
    nodes.get(".dial__small")!.textContent = "default illustration";
    nodes.get(".dial__segments")!.children = [new Element(), new Element(), new Element()];
    vi.stubGlobal("document", doc);
    initTimelock();
    const event = new Event("gate-config-observed") as Event & { detail: { governance_delay_days: number } };
    event.detail = { governance_delay_days: days };
    doc.dispatchEvent(event);
    expect(nodes.get(".dial__big")!.textContent).toBe(`${days} ${days === 1 ? "day" : "days"}`);
    expect(nodes.get(".dial__segments")!.children).toHaveLength(days);
    expect(nodes.get(".dial__small")!.textContent).toContain("observed");
    expect(nodes.get(".dial__description")!.textContent).toContain(String(days));
  });
});
