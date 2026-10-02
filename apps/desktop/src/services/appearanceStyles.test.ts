import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const stylesheet = readFileSync(new URL("../styles.css", import.meta.url), "utf8");

function rule(selector: string): string {
  const match = stylesheet.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`));
  return match?.[1] ?? "";
}

describe("appearance stylesheet", () => {
  it.each(["indigo", "violet", "amber", "rose", "gray"])("gives %s its own structural light colors", (theme) => {
    const content = rule(`:root[data-color-theme="${theme}"]`);
    expect(content).toContain("--titlebar:");
    expect(content).toContain("--sidebar:");
    expect(content).toContain("--app-background:");
  });

  it.each(["teal", "indigo", "violet", "amber", "rose", "gray"])("keeps %s dark-mode icons legible", (theme) => {
    const selector = theme === "teal"
      ? ':root[data-color-mode="dark"]'
      : `:root[data-color-mode="dark"][data-color-theme="${theme}"]`;
    expect(rule(selector)).toContain("--icon:");
  });

  it("uses the bright titlebar foreground for idle toolbar icons", () => {
    expect(rule(".toolbar-action")).toContain("color: var(--titlebar-text)");
  });

  it("keeps the active mode icon white on every theme", () => {
    expect(rule(".floating-theme-toggle.is-dark")).toContain("color: #ffffff");
  });

  it("uses the titlebar foreground for the brand in dark mode", () => {
    expect(rule(".brand")).toContain("color: var(--titlebar-text)");
  });

  it("visually centers titlebar status dots with their labels", () => {
    expect(rule(".sync-state i")).toContain("transform: translateY(1px)");
  });

  it("keeps the archive relocation warning readable on every theme", () => {
    const content = rule(".archive-row.needs-location");
    expect(content).toContain("background: var(--warning-soft)");
    expect(content).not.toContain("#fff4d6");
  });

  it("defines the warning palette for both color modes", () => {
    expect(rule(":root")).toContain("--warning-soft:");
    expect(rule(':root[data-color-mode="dark"]')).toContain("--warning-soft:");
  });

  it("never remaps list chips to a light-only background in dark mode", () => {
    const darkOverrides = stylesheet.slice(stylesheet.indexOf(':root[data-color-mode="dark"] .sort-menu'));
    expect(darkOverrides).toContain(".automation-badge");
    expect(darkOverrides).toContain(".row-title i.synced");
  });
});

describe("theme variables", () => {
  const sourceDirectory = new URL("../", import.meta.url);
  const sources = readdirSync(sourceDirectory, { recursive: true, encoding: "utf8" })
    .filter((name) => /\.(css|vue|ts)$/.test(name) && !name.endsWith(".test.ts"))
    .map((name) => readFileSync(new URL(name.replaceAll("\\", "/"), sourceDirectory), "utf8"))
    .join("\n");
  // Inline style bindings declare their custom properties with a quoted key.
  const declared = new Set([...sources.matchAll(/(--[a-zA-Z][\w-]*)['"]?\s*:/g)].map((match) => match[1]));
  const used = new Set([...sources.matchAll(/var\(\s*(--[a-zA-Z][\w-]*)/g)].map((match) => match[1]));

  it("resolves every custom property to a declaration", () => {
    // A `var()` fallback is not enough: an undefined variable silently freezes a
    // light-only color into the component and breaks dark mode contrast.
    expect([...used].filter((name) => !declared.has(name))).toEqual([]);
  });
});
