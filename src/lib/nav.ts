export type Dir = "up" | "down" | "left" | "right";

// Last non-sidebar element that received focus. Used to restore focus when
// the user exits the sidebar via Right.
let lastMainFocus: HTMLElement | null = null;
let initialized = false;

/** Register the focus tracker. Call once on app mount. */
export function initNav(): void {
  if (initialized) return;
  initialized = true;
  document.addEventListener("focusin", (e) => {
    const el = e.target;
    if (!(el instanceof HTMLElement)) return;
    const zone = getZone(el);
    if (zone && zone !== "sidebar") {
      lastMainFocus = el;
    }
  });
}

/** The nav zone of the given element, read from the nearest
 *  `[data-nav-zone]` ancestor. */
function getZone(el: Element | null): string | null {
  if (!el) return null;
  const zone = (el as HTMLElement).closest<HTMLElement>("[data-nav-zone]");
  return zone?.dataset.navZone ?? null;
}

function isVisible(el: HTMLElement): boolean {
  if (el.offsetParent === null && getComputedStyle(el).position !== "fixed") {
    return false;
  }
  const style = getComputedStyle(el);
  return style.visibility !== "hidden" && style.display !== "none";
}

function focusablesIn(scope: ParentNode): HTMLElement[] {
  return Array.from(
    scope.querySelectorAll<HTMLElement>(
      "button:not([disabled]):not([data-no-nav])",
    ),
  ).filter((el) => isVisible(el));
}

function focusablesInZone(zone: string): HTMLElement[] {
  const root = document.querySelector<HTMLElement>(
    `[data-nav-zone="${zone}"]`,
  );
  if (!root) return [];
  return focusablesIn(root);
}

/** Nearest candidate in `dir` that satisfies `accept`. Uses a directional
 *  cone: the candidate must be more in the requested direction than off-axis
 *  (|primary| > |perp|) so "up" doesn't mean "anywhere above and to the side". */
function spatialNearest(
  current: HTMLElement,
  dir: Dir,
  accept: (el: HTMLElement) => boolean,
): HTMLElement | null {
  const candidates = focusablesIn(document).filter(
    (el) => el !== current && accept(el),
  );
  if (candidates.length === 0) return null;

  const cur = current.getBoundingClientRect();
  const cxCur = cur.left + cur.width / 2;
  const cyCur = cur.top + cur.height / 2;

  let best: HTMLElement | null = null;
  let bestScore = Infinity;

  for (const el of candidates) {
    const r = el.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const dx = cx - cxCur;
    const dy = cy - cyCur;

    let primary: number;
    let perp: number;
    switch (dir) {
      case "up":
        if (dy >= -1) continue;
        if (Math.abs(dy) < Math.abs(dx)) continue;
        primary = -dy;
        perp = Math.abs(dx);
        break;
      case "down":
        if (dy <= 1) continue;
        if (Math.abs(dy) < Math.abs(dx)) continue;
        primary = dy;
        perp = Math.abs(dx);
        break;
      case "left":
        if (dx >= -1) continue;
        if (Math.abs(dx) < Math.abs(dy)) continue;
        primary = -dx;
        perp = Math.abs(dy);
        break;
      case "right":
        if (dx <= 1) continue;
        if (Math.abs(dx) < Math.abs(dy)) continue;
        primary = dx;
        perp = Math.abs(dy);
        break;
    }
    const score = primary + perp * 2;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }

  return best;
}

/** Nearest ancestor that is an actual horizontal scroll container
 *  (overflow-x: auto | scroll with real overflow). We deliberately skip
 *  overflow:hidden ancestors — browsers still treat them as scrollable by
 *  `scrollIntoView`, which caused the hero content to drift left when the
 *  carousel had many items. */
function nearestHScrollContainer(el: HTMLElement): HTMLElement | null {
  let p = el.parentElement;
  while (p) {
    const ox = getComputedStyle(p).overflowX;
    if ((ox === "auto" || ox === "scroll") && p.scrollWidth > p.clientWidth) {
      return p;
    }
    p = p.parentElement;
  }
  return null;
}

function focusEl(el: HTMLElement) {
  el.focus({ preventScroll: true });

  // Horizontally center the element inside the nearest scroll container only.
  // This prevents ancestor containers from also trying to "center" the card,
  // which would push the hero (logo / Play button / stats) off-screen.
  const hs = nearestHScrollContainer(el);
  if (hs) {
    const elRect = el.getBoundingClientRect();
    const boxRect = hs.getBoundingClientRect();
    const target =
      hs.scrollLeft +
      (elRect.left + elRect.width / 2) -
      (boxRect.left + boxRect.width / 2);
    hs.scrollTo({ left: target, behavior: "smooth" });
  }

  // Vertical: non-centering, only scrolls if off-screen (e.g., settings page).
  el.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
}

/**
 * TV-style zoned focus navigator.
 *
 * Zones are declared by `data-nav-zone="..."` on container elements.
 * Rules:
 *   - Inside `sidebar`: up/down cycle within the sidebar, right exits to
 *     the last-focused main element (or spatial fallback), left is a no-op.
 *   - Inside any other zone: nav is constrained to that zone. If `left`
 *     has no in-zone candidate, focus enters the sidebar and targets the
 *     active item (the one marked `.active`), falling back to the first.
 *     Other directions with no in-zone candidate are a no-op.
 */
export function navigate(dir: Dir): void {
  const current = document.activeElement;
  if (
    !(current instanceof HTMLElement) ||
    !(current instanceof HTMLButtonElement) ||
    current.disabled
  ) {
    const first = focusablesIn(document)[0];
    if (first) focusEl(first);
    return;
  }

  const zone = getZone(current);

  // ---------- Sidebar ----------
  if (zone === "sidebar") {
    if (dir === "up" || dir === "down") {
      const items = focusablesInZone("sidebar");
      if (items.length === 0) return;
      const idx = items.indexOf(current);
      if (idx < 0) {
        items[0].focus({ preventScroll: true });
        return;
      }
      const next =
        dir === "up"
          ? (idx - 1 + items.length) % items.length
          : (idx + 1) % items.length;
      items[next].focus({ preventScroll: true });
      return;
    }
    if (dir === "right") {
      if (
        lastMainFocus &&
        document.contains(lastMainFocus) &&
        isVisible(lastMainFocus)
      ) {
        focusEl(lastMainFocus);
        return;
      }
      const target = spatialNearest(
        current,
        "right",
        (el) => getZone(el) !== "sidebar",
      );
      if (target) focusEl(target);
      return;
    }
    // Left inside sidebar: no-op.
    return;
  }

  // ---------- Modal menu: cycle up/down, all other directions no-op ----------
  if (zone === "menu") {
    if (dir === "up" || dir === "down") {
      const items = focusablesInZone("menu");
      if (items.length === 0) return;
      const idx = items.indexOf(current);
      if (idx < 0) {
        items[0].focus({ preventScroll: true });
        return;
      }
      const next =
        dir === "up"
          ? (idx - 1 + items.length) % items.length
          : (idx + 1) % items.length;
      items[next].focus({ preventScroll: true });
    }
    return;
  }

  // ---------- Any other zone: constrain nav to that zone ----------
  if (zone) {
    const inZone = spatialNearest(
      current,
      dir,
      (el) => getZone(el) === zone,
    );
    if (inZone) {
      focusEl(inZone);
      return;
    }
    // Left is the only escape hatch — lands on the sidebar's active page.
    if (dir === "left") {
      const active = document.querySelector<HTMLElement>(
        `[data-nav-zone="sidebar"] button.active`,
      );
      const fallback = focusablesInZone("sidebar")[0];
      const target = active ?? fallback;
      target?.focus({ preventScroll: true });
    }
  }
}
