import type { SurfaceView } from "../../lib/surface-view.ts";
import { text } from "./runtime.ts";

/** Marker glyphs shared with the StageLadder and Marker components. */
export const markerPaths = {
  done: "M3.5 8.4 6.6 11.3 12.5 4.9",
  refused: "M4 4 12 12M12 4 4 12",
  skipped: "M3.5 8h9",
} as const;

/** Updates a server-rendered StageLadder in place from a surface view; the row count never changes. */
export function renderLadder(root: HTMLElement, stages: SurfaceView["stages"]): void {
  const rows = root.querySelectorAll<HTMLElement>(".ladder__row");
  stages.forEach((stage, index) => {
    const row = rows[index];
    if (!row) return;
    row.classList.remove("is-pending", "is-active", "is-done", "is-refused", "is-skipped");
    row.classList.add(`is-${stage.state}`);
    text(row, ".ladder__label", stage.label);
    text(
      row,
      ".ladder__status",
      stage.status ??
        {
          pending: "Pending",
          active: "In progress",
          done: "Done",
          refused: "Refused",
          skipped: "Not reached",
        }[stage.state],
    );
    let detail = row.querySelector<HTMLElement>(".ladder__detail");
    if (!detail) {
      detail = document.createElement("span");
      detail.className = "ladder__detail";
      row.append(detail);
    }
    detail.textContent = stage.detail ?? "";
    const marker = row.querySelector<SVGElement>(".marker");
    if (!marker) return;
    marker.classList.remove(
      "marker--pending",
      "marker--active",
      "marker--done",
      "marker--refused",
      "marker--skipped",
    );
    marker.classList.add(`marker--${stage.state}`);
    marker.replaceChildren();
    if (stage.state === "pending" || stage.state === "active") {
      const circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      circle.setAttribute("cx", "8");
      circle.setAttribute("cy", "8");
      circle.setAttribute("r", "6");
      marker.append(circle);
      if (stage.state === "active") {
        const dot = circle.cloneNode() as SVGCircleElement;
        dot.setAttribute("r", "2.75");
        dot.setAttribute("fill", "currentColor");
        marker.append(dot);
      }
    } else {
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", markerPaths[stage.state]);
      marker.append(path);
    }
  });
}
