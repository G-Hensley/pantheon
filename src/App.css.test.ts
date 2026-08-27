import { describe, expect, it } from "vitest";
import css from "./App.css?raw";

// A guard on the stylesheet itself, because the regression it prevents is a
// layout fact that jsdom cannot see: jsdom has no layout engine, so no unit
// test in this suite can observe a scrollbar. What a unit test *can* do is
// pin the rules the measured fix depends on.
//
// The regression: `.cond-feed { overflow-x: auto }` lived in the conductor
// bar, the fixed-height strip directly under the title bar. Measured in
// WebView2 at 1477x427, five task pills overflowed it (1405px of content in
// 955px of box) and it painted a classic 15px horizontal scrollbar at y=82,
// growing the bar from 43px to 49px. Windows scrollbars take space rather than
// overlaying, so the strip is exactly where the user sees it. It reproduced at
// 1024, 1366, 1477 and 1920 wide, and cleared only at 2560.
//
// Two rules matter, and one tempting rule does not:
//   - nothing inside the top chrome may open a scroll container;
//   - `body { overflow: hidden }` keeps the page itself from scrolling;
//   - clipping the *parent* strip does nothing. A child with its own
//     `overflow-x: auto` still paints its own scrollbar inside a clipped
//     parent. That was measured too, so this file checks the children.

type Rule = { selector: string; body: string };

function rules(source: string): Rule[] {
  // Comments go first: a declaration inside one is documentation, not CSS.
  const stripped = source.replace(/\/\*[\s\S]*?\*\//g, "");
  const out: Rule[] = [];
  const re = /([^{}]+)\{([^{}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(stripped)) !== null) {
    out.push({ selector: match[1].trim().replace(/\s+/g, " "), body: match[2] });
  }
  return out;
}

// Every class that lives inside the fixed-height strips across the top of the
// window. `.cond-*` is listed as a namespace on purpose: the selector that
// caused the regression was `.cond-feed`, which never mentions `.condbar` at
// all, so matching on the parent's name would have missed the very bug this
// guards against.
const TOP_CHROME = /(^|[\s,>+~(])\.(bar|restore-problem|condbar|cond-[a-z0-9-]+)(?![\w-])/;

// Any axis, because `overflow-y: auto` alone is enough: with the other axis
// left at `visible`, CSS computes it to `auto` too, which is how a vertical
// scroll container quietly becomes a horizontal one.
const OPENS_A_SCROLL_CONTAINER = /overflow(-x|-y)?\s*:\s*(auto|scroll)/;

const ALL = rules(css);
const topChrome = ALL.filter((r) => TOP_CHROME.test(r.selector));

function bodyOf(selector: string): string {
  const rule = ALL.find((r) => r.selector === selector);
  if (!rule) throw new Error(`no rule for selector ${selector}`);
  return rule.body;
}

describe("App.css top chrome", () => {
  it("parses into rules at all, so an empty match below cannot pass vacuously", () => {
    expect(ALL.length).toBeGreaterThan(100);
    expect(topChrome.length).toBeGreaterThan(5);
  });

  it("opens no scroll container anywhere in the title bar or conductor bar", () => {
    const offenders = topChrome
      .filter((r) => OPENS_A_SCROLL_CONTAINER.test(r.body))
      .map((r) => r.selector);
    expect(offenders).toEqual([]);
  });

  it("would have caught the rule that caused the regression", () => {
    // The guard is only worth having if it fails on the real thing, so check it
    // against the exact rule that shipped, rather than trusting the regexes.
    const regression = rules(".cond-feed {\n  display: flex;\n  overflow-x: auto;\n  min-width: 0;\n}");
    expect(regression).toHaveLength(1);
    expect(TOP_CHROME.test(regression[0].selector)).toBe(true);
    expect(OPENS_A_SCROLL_CONTAINER.test(regression[0].body)).toBe(true);
  });

  it("does not mistake an unrelated class for top chrome", () => {
    // `.grid` scrolls by design and must stay allowed; nothing whose name
    // merely ends in "bar" counts either.
    expect(TOP_CHROME.test('.grid[data-layout="fit"]')).toBe(false);
    expect(TOP_CHROME.test(".side-list")).toBe(false);
    expect(TOP_CHROME.test(".taskbar")).toBe(false);
    expect(TOP_CHROME.test(".pane-bar")).toBe(false);
  });

  it("clips each strip, so none of them can widen or scroll the page", () => {
    const clipped = bodyOf(".bar, .restore-problem, .condbar");
    expect(clipped).toMatch(/overflow-x\s*:\s*clip/);
  });

  it("keeps the page itself unscrollable", () => {
    // The done_when is "no horizontal page scrollbar at 1477x427". This is the
    // rule that delivers the "page" half; the checks above deliver the rest.
    expect(bodyOf("body")).toMatch(/overflow\s*:\s*hidden/);
  });

  it("lets the conductor's name give way instead of pushing the bar wider", () => {
    // Something in the strip has to absorb the pressure or the buttons get
    // pushed off the window edge, which is the same bug wearing a hat.
    const title = bodyOf(".cond-title");
    expect(title).toMatch(/min-width\s*:\s*0/);
    expect(title).toMatch(/text-overflow\s*:\s*ellipsis/);
  });

  it("keeps button labels on one line, so the strip cannot grow taller instead", () => {
    expect(bodyOf(".bar button, .condbar button")).toMatch(/white-space\s*:\s*nowrap/);
  });
});

describe("App.css delivery-state pills", () => {
  // increment-1.md: "task status and attention states ... do not rely on color
  // alone". The word in the pill comes from statusLabel() and is covered in
  // src/lib/tasks.test.ts; this is the visual half. queued, submitted and
  // accepted must differ in something a greyscale screen still shows, which
  // here is fill: none, faint, solid.
  const pill = (status: string) => bodyOf(`.taskitem-status[data-status="${status}"]`);

  it("distinguishes queued by border style, not by colour", () => {
    expect(pill("queued")).toMatch(/border-style\s*:\s*dashed/);
  });

  it("distinguishes submitted and accepted by fill, not by colour", () => {
    expect(pill("submitted")).toMatch(/background\s*:/);
    expect(pill("accepted")).toMatch(/background\s*:\s*var\(--acc\)/);
  });

  it("gives the three delivery states three different treatments", () => {
    const bodies = ["queued", "submitted", "accepted"].map((s) => pill(s).replace(/\s+/g, " ").trim());
    expect(new Set(bodies).size).toBe(3);
  });

  it("styles every status the drawer can render, so none falls back to a bare pill", () => {
    const styled = new Set(
      ALL.map((r) => /\.taskitem-status\[data-status="([a-z_]+)"\]/.exec(r.selector)?.[1]).filter(
        Boolean,
      ),
    );
    for (const status of [
      "queued",
      "submitted",
      "accepted",
      "pending",
      "overdue",
      "rework",
      "blocked",
      "in_review",
      "done",
      "error",
      "cancelled",
      "abandoned",
    ]) {
      expect(styled).toContain(status);
    }
  });
});
