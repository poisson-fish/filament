import { fireEvent, render, screen } from "@solidjs/testing-library";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MarkdownToken } from "../src/domain/chat";
import { SafeMarkdown } from "../src/features/app-shell/components/SafeMarkdown";

function renderMarkdown(tokens: MarkdownToken[]): void {
  render(() => <SafeMarkdown tokens={tokens} />);
}

describe("safe markdown", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("renders safe links and blocks obfuscated javascript/data links", () => {
    renderMarkdown([
      { type: "link_start", href: "https://filament.test/docs" },
      { type: "text", text: "docs" },
      { type: "link_end" },
      { type: "text", text: " " },
      { type: "link_start", href: "javascript:alert(1)" },
      { type: "text", text: "pwnd" },
      { type: "link_end" },
      { type: "text", text: " " },
      { type: "link_start", href: "data:text/html,<script>alert(1)</script>" },
      { type: "text", text: "data" },
      { type: "link_end" },
      { type: "text", text: " " },
      { type: "link_start", href: "JaVaScRiPt:alert(2)" },
      { type: "text", text: "mixed-case-js" },
      { type: "link_end" },
      { type: "text", text: " " },
      { type: "link_start", href: "DATA:text/html;base64,PHNjcmlwdA==" },
      { type: "text", text: "mixed-case-data" },
      { type: "link_end" },
      { type: "text", text: " " },
      { type: "link_start", href: "MAILTO:admin@filament.test" },
      { type: "text", text: "mail" },
      { type: "link_end" },
    ]);

    const safeLink = screen.getByRole("link", { name: "docs" });
    expect(safeLink).toHaveAttribute("href", "https://filament.test/docs");
    expect(safeLink).toHaveAttribute("target", "_blank");
    expect(safeLink).toHaveAttribute("rel", "noopener noreferrer");
    const mailLink = screen.getByRole("link", { name: "mail" });
    expect(mailLink).toHaveAttribute("href", "mailto:admin@filament.test");
    expect(screen.queryByRole("link", { name: "pwnd" })).toBeNull();
    expect(screen.queryByRole("link", { name: "data" })).toBeNull();
    expect(screen.queryByRole("link", { name: "mixed-case-js" })).toBeNull();
    expect(screen.queryByRole("link", { name: "mixed-case-data" })).toBeNull();
  });

  it("opens a confirmation modal before opening user-submitted links", () => {
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    renderMarkdown([
      { type: "link_start", href: "https://filament.test/docs" },
      { type: "text", text: "docs" },
      { type: "link_end" },
    ]);

    fireEvent.click(screen.getByRole("link", { name: "docs" }));

    expect(screen.getByRole("dialog", { name: "External link confirmation" })).toBeInTheDocument();
    expect(screen.getByText("https://filament.test/docs")).toBeInTheDocument();
    expect(openSpy).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Visit Site" }));
    expect(openSpy).toHaveBeenCalledWith(
      "https://filament.test/docs",
      "_blank",
      "noopener,noreferrer",
    );
    expect(screen.queryByRole("dialog", { name: "External link confirmation" })).toBeNull();
  });

  it("requires confirmation for every activation and never bypasses future prompts", () => {
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    renderMarkdown([
      { type: "link_start", href: "https://filament.test/docs" },
      { type: "text", text: "docs" },
      { type: "link_end" },
    ]);

    fireEvent.click(screen.getByRole("link", { name: "docs" }));
    fireEvent.click(screen.getByRole("button", { name: "Visit Site" }));
    openSpy.mockClear();

    fireEvent.click(screen.getByRole("link", { name: "docs" }));
    expect(screen.getByRole("dialog", { name: "External link confirmation" })).toBeInTheDocument();
    expect(openSpy).not.toHaveBeenCalled();
  });

  it("normalizes destinations before confirmation/open and rejects credentialed links", () => {
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    renderMarkdown([
      { type: "link_start", href: "HTTPS://Filament.test:443/Docs" },
      { type: "text", text: "normalized" },
      { type: "link_end" },
      { type: "text", text: " " },
      { type: "link_start", href: "https://user:pass@filament.test/private" },
      { type: "text", text: "credentialed" },
      { type: "link_end" },
    ]);

    fireEvent.click(screen.getByRole("link", { name: "normalized" }));
    expect(screen.getByText("https://filament.test/Docs")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Visit Site" }));
    expect(openSpy).toHaveBeenCalledWith(
      "https://filament.test/Docs",
      "_blank",
      "noopener,noreferrer",
    );
    expect(screen.queryByRole("link", { name: "credentialed" })).toBeNull();
  });

  it("routes auxiliary-clicked links through confirmation instead of direct browser navigation", () => {
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    renderMarkdown([
      { type: "link_start", href: "https://filament.test/docs" },
      { type: "text", text: "docs" },
      { type: "link_end" },
    ]);

    fireEvent(
      screen.getByRole("link", { name: "docs" }),
      new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 }),
    );
    expect(screen.getByRole("dialog", { name: "External link confirmation" })).toBeInTheDocument();
    expect(openSpy).not.toHaveBeenCalled();
  });

  it("treats raw html as inert text and never emits script nodes", () => {
    renderMarkdown([{ type: "text", text: "<script>alert(1)</script>" }]);
    expect(screen.getByText("<script>alert(1)</script>")).toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
  });

  it("renders emoji content as twemoji sprite spans", () => {
    renderMarkdown([{ type: "text", text: "hello 😂" }]);
    const emojiSprite = screen.getByRole("img", { name: "😂" });
    const style = emojiSprite.getAttribute("style") ?? "";
    expect(style).toContain("twitter-sheets-256-64.png");
  });

  it("renders fenced code labels and downgrades invalid labels to plain text", () => {
    renderMarkdown([
      { type: "fenced_code", language: "rust", code: "fn main() {}\n" },
      { type: "fenced_code", language: "rust<script>", code: "alert(1)" },
    ]);

    const labels = [...document.querySelectorAll(".safe-markdown-code-label")].map((node) =>
      node.textContent?.trim(),
    );
    expect(labels).toContain("rust");
    expect(labels).toContain("code");
    expect(screen.getByText("fn")).toBeInTheDocument();
    expect(screen.getByText("alert(1)")).toBeInTheDocument();
  });

  it("renders syntax token spans for fenced code languages", () => {
    renderMarkdown([{ type: "fenced_code", language: "rust", code: "fn main() {}\n" }]);
    expect(document.querySelector(".safe-markdown-code-block .hljs-keyword")).not.toBeNull();
  });

  it("renders heading tokens as semantic heading elements", () => {
    renderMarkdown([
      { type: "heading_start", level: 1 },
      { type: "text", text: "Incident report" },
      { type: "heading_end" },
      { type: "heading_start", level: 3 },
      { type: "text", text: "Timeline" },
      { type: "heading_end" },
    ]);

    expect(screen.getByRole("heading", { level: 1, name: "Incident report" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 3, name: "Timeline" })).toBeInTheDocument();
  });
});
