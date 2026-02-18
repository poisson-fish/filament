// @vitest-environment node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const testDir = resolve(fileURLToPath(new URL(".", import.meta.url)));
const webRootDir = resolve(testDir, "..");
const shellRefreshCssPath = resolve(webRootDir, "src/styles/app/shell-refresh.css");

function extractRules(cssSource: string, selector: string): string[] {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return [...cssSource.matchAll(new RegExp(`${escapedSelector}\\s*\\{([\\s\\S]*?)\\}`, "g"))]
    .map((match) => match[1] ?? "");
}

function expectRuleWithDeclarations(
  cssSource: string,
  selector: string,
  declarationPatterns: RegExp[],
): string {
  const rules = extractRules(cssSource, selector);
  expect(rules.length).toBeGreaterThan(0);

  const matchingRule = rules.find((rule) =>
    declarationPatterns.every((pattern) => pattern.test(rule))
  );
  expect(matchingRule).toBeDefined();
  return matchingRule ?? "";
}

describe("app shell chat layout contract", () => {
  it("pins the composer by reserving a trailing chat-panel row", () => {
    const shellCss = readFileSync(shellRefreshCssPath, "utf8");
    expectRuleWithDeclarations(shellCss, ".chat-panel", [
      /display:\s*grid;/,
      /grid-template-rows:\s*auto\s+1fr\s+auto;/,
      /height:\s*100%;/,
      /min-height:\s*0;/,
      /overflow:\s*hidden;/,
    ]);
  });

  it("keeps message scrolling isolated above the composer", () => {
    const shellCss = readFileSync(shellRefreshCssPath, "utf8");
    expectRuleWithDeclarations(shellCss, ".chat-body", [
      /display:\s*flex;/,
      /flex-direction:\s*column;/,
      /min-height:\s*0;/,
      /overflow:\s*hidden;/,
    ]);

    expectRuleWithDeclarations(shellCss, ".chat-body .message-list", [
      /flex:\s*1\s+1\s+0;/,
      /min-height:\s*0;/,
      /overflow-y:\s*auto;/,
    ]);

    expectRuleWithDeclarations(shellCss, ".chat-panel > .composer", [
      /margin-top:\s*0;/,
      /min-height:\s*0;/,
    ]);
  });
});
