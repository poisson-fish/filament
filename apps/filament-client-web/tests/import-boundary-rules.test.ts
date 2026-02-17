import { describe, expect, it } from "vitest";
// @ts-expect-error typed via runtime Node script contract.
import { classifySourceLayer, collectImportBoundaryViolations, extractImportSpecifiers, resolveProjectImportTarget } from "../scripts/import-boundary-rules.mjs";

describe("import boundary rules", () => {
  it("classifies source layers from src-relative paths", () => {
    expect(classifySourceLayer("src/domain/auth.ts")).toBe("domain");
    expect(classifySourceLayer("src/lib/api.ts")).toBe("lib");
    expect(classifySourceLayer("src/features/app-shell/helpers.ts")).toBe("features");
    expect(classifySourceLayer("tests/import-boundary-rules.test.ts")).toBeNull();
  });

  it("extracts import and export-from specifiers", () => {
    const source = [
      'import { x } from "../lib/api";',
      'export { y } from "../../features/app-shell/types";',
      'import "./polyfill";',
    ].join("\n");

    expect(extractImportSpecifiers(source)).toEqual([
      "../lib/api",
      "../../features/app-shell/types",
      "./polyfill",
    ]);
  });

  it("resolves relative and src-absolute import targets", () => {
    expect(
      resolveProjectImportTarget({
        projectPath: "src/domain/chat.ts",
        importSpecifier: "../lib/api",
      }),
    ).toBe("src/lib/api");

    expect(
      resolveProjectImportTarget({
        projectPath: "src/features/app-shell/helpers.ts",
        importSpecifier: "src/lib/rtc",
      }),
    ).toBe("src/lib/rtc");

    expect(
      resolveProjectImportTarget({
        projectPath: "src/features/app-shell/helpers.ts",
        importSpecifier: "solid-js",
      }),
    ).toBeNull();
  });

  it("reports forbidden boundary violations and keeps allowed directions", () => {
    const violations = collectImportBoundaryViolations([
      {
        path: "src/domain/auth.ts",
        content: 'import { fetchHealth } from "../lib/api";',
      },
      {
        path: "src/lib/api.ts",
        content: 'import { createAppShellRuntime } from "../features/app-shell/runtime/create-app-shell-runtime";',
      },
      {
        path: "src/features/app-shell/helpers.ts",
        content: 'import { ApiError } from "../../lib/api";',
      },
      {
        path: "src/lib/rtc.ts",
        content: 'import { userIdFromInput } from "../domain/chat";',
      },
    ]);

    expect(violations).toEqual([
      {
        filePath: "src/domain/auth.ts",
        importSpecifier: "../lib/api",
        sourceLayer: "domain",
        targetLayer: "lib",
        reason: "domain must not import lib",
      },
      {
        filePath: "src/lib/api.ts",
        importSpecifier: "../features/app-shell/runtime/create-app-shell-runtime",
        sourceLayer: "lib",
        targetLayer: "features",
        reason: "lib must not import features",
      },
    ]);
  });
});
