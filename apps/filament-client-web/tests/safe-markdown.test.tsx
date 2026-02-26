import { render, screen } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import type { MarkdownToken } from "../src/domain/chat";
import { SafeMarkdown } from "../src/features/app-shell/components/SafeMarkdown";

function renderMarkdown(tokens: MarkdownToken[]): void {
  render(() => <SafeMarkdown tokens={tokens} />);
}

describe("safe markdown", () => {
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
    expect(mailLink).toHaveAttribute("href", "MAILTO:admin@filament.test");
    expect(screen.queryByRole("link", { name: "pwnd" })).toBeNull();
    expect(screen.queryByRole("link", { name: "data" })).toBeNull();
    expect(screen.queryByRole("link", { name: "mixed-case-js" })).toBeNull();
    expect(screen.queryByRole("link", { name: "mixed-case-data" })).toBeNull();
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
    expect(labels).toContain("```rust");
    expect(labels).toContain("```");
    expect(screen.getByText("fn")).toBeInTheDocument();
    expect(screen.getByText("alert(1)")).toBeInTheDocument();
  });
});
