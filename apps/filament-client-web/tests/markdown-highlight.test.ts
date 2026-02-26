import { describe, expect, it } from "vitest";

import {
  ALLOWED_FENCED_CODE_LANGUAGES,
  createFilamentMarkdownHighlighter,
  resolveHighlightLanguage,
} from "../src/features/app-shell/components/markdown-highlight";

function collectNodeTypes(node: unknown, into: string[]): void {
  if (!node || typeof node !== "object") {
    return;
  }
  const record = node as { type?: unknown; children?: unknown };
  if (typeof record.type === "string") {
    into.push(record.type);
  }
  if (Array.isArray(record.children)) {
    for (const child of record.children) {
      collectNodeTypes(child, into);
    }
  }
}

describe("markdown-highlight", () => {
  it("registers a bounded language allowlist", () => {
    const lowlight = createFilamentMarkdownHighlighter();
    const registered = lowlight.listLanguages().sort();
    expect(registered).toEqual([...ALLOWED_FENCED_CODE_LANGUAGES].sort());
  });

  it("normalizes and resolves safe language labels", () => {
    expect(resolveHighlightLanguage("RuSt")).toBe("rust");
    expect(resolveHighlightLanguage("ts")).toBe("typescript");
    expect(resolveHighlightLanguage("html")).toBe("xml");
    expect(resolveHighlightLanguage("unknown")).toBeNull();
    expect(resolveHighlightLanguage("rust<script>")).toBeNull();
  });

  it("produces AST nodes without raw html node types", () => {
    const lowlight = createFilamentMarkdownHighlighter();
    const tree = lowlight.highlight(
      "javascript",
      "const value = '<script>alert(1)</script>';",
    );
    const nodeTypes: string[] = [];
    collectNodeTypes(tree, nodeTypes);
    expect(nodeTypes).toContain("root");
    expect(nodeTypes).not.toContain("raw");
    expect(nodeTypes).not.toContain("html");
  });
});

