/** Real DOM visual / accessibility checks for generated surfaces. */

export type VisualCheckStatus = "pass" | "warn" | "fail";

export type VisualCheck = {
  id: string;
  status: VisualCheckStatus;
  detail?: string;
};

export type VisualVerificationResult = {
  width: number;
  checks: VisualCheck[];
  implemented: true;
};

const MIN_TOUCH = 24;

function hasAccessibleName(el: Element): boolean {
  const aria = el.getAttribute("aria-label")?.trim();
  if (aria) return true;
  const labelled = el.getAttribute("aria-labelledby");
  if (labelled && document.getElementById(labelled)?.textContent?.trim()) return true;
  if (
    el instanceof HTMLInputElement ||
    el instanceof HTMLTextAreaElement ||
    el instanceof HTMLSelectElement
  ) {
    if (el.labels && el.labels.length > 0) return true;
    if (el.getAttribute("placeholder")?.trim()) return true;
    if (el.closest("label")?.textContent?.trim()) return true;
    if (el.id && document.querySelector(`label[for="${el.id}"]`)?.textContent?.trim()) return true;
    if (el.getAttribute("title")?.trim()) return true;
  }
  if (el instanceof HTMLButtonElement || el.getAttribute("role") === "button") {
    if (el.textContent?.trim()) return true;
    if (el.getAttribute("title")?.trim()) return true;
  }
  if (el instanceof HTMLImageElement) {
    return Boolean(el.getAttribute("alt")?.trim());
  }
  return false;
}

/**
 * Inspect a rendered surface root for overflow, labels, and touch targets.
 */
export function verifySurfaceElement(
  root: HTMLElement,
  width = root.clientWidth || window.innerWidth,
): VisualVerificationResult {
  const checks: VisualCheck[] = [];

  const overflowX = root.scrollWidth > root.clientWidth + 2;
  checks.push({
    id: "no_horizontal_overflow",
    status: overflowX ? "fail" : "pass",
    detail: overflowX
      ? `scrollWidth ${root.scrollWidth} > clientWidth ${root.clientWidth}`
      : undefined,
  });

  const interactive = Array.from(
    root.querySelectorAll(
      "button, a, input, textarea, select, [role='button'], [tabindex]",
    ),
  ).filter((node) => {
    if (!(node instanceof HTMLElement)) return false;
    const tabindex = node.getAttribute("tabindex");
    if (tabindex === "-1") return false;
    return true;
  });
  let missingLabel = 0;
  let smallTarget = 0;
  interactive.forEach((node) => {
    if (!(node instanceof HTMLElement)) return;
    if (!hasAccessibleName(node)) missingLabel += 1;
    const rect = node.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      if (Math.min(rect.width, rect.height) < MIN_TOUCH) smallTarget += 1;
    }
  });

  checks.push({
    id: "labels_present",
    status: missingLabel > 0 ? "fail" : "pass",
    detail: missingLabel > 0 ? `${missingLabel} controls lack accessible names` : undefined,
  });

  checks.push({
    id: "touch_targets",
    status: smallTarget > 0 ? (width < 400 ? "fail" : "warn") : "pass",
    detail: smallTarget > 0 ? `${smallTarget} controls below ${MIN_TOUCH}px` : undefined,
  });

  const clipped = Array.from(root.querySelectorAll("*")).some((node) => {
    if (!(node instanceof HTMLElement)) return false;
    const style = getComputedStyle(node);
    if (style.overflow === "hidden" || style.textOverflow === "ellipsis") {
      return node.scrollWidth > node.clientWidth + 1;
    }
    return false;
  });
  checks.push({
    id: "text_clipping",
    status: clipped ? "warn" : "pass",
  });

  const charts = root.querySelectorAll("[data-chart], .chart, canvas");
  let emptyChart = 0;
  charts.forEach((node) => {
    if (node instanceof HTMLCanvasElement && node.width * node.height === 0) emptyChart += 1;
    if (node.getAttribute("data-empty") === "true") emptyChart += 1;
  });
  checks.push({
    id: "empty_chart_state",
    status: emptyChart > 0 ? "warn" : "pass",
    detail: emptyChart > 0 ? `${emptyChart} empty chart surfaces` : undefined,
  });

  return { width, checks, implemented: true };
}
