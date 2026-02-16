import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, describe, expect, it } from "vitest";
import { runImportBoundaryCheck } from "../scripts/check-import-boundaries.mjs";

function writeFixture(webRootDir: string, relativePath: string, content: string): void {
  const absolutePath = join(webRootDir, relativePath);
  mkdirSync(dirname(absolutePath), { recursive: true });
  writeFileSync(absolutePath, content, "utf8");
}

describe("check import boundaries", () => {
  const temporaryRoots: string[] = [];

  afterEach(() => {
    for (const rootPath of temporaryRoots) {
      rmSync(rootPath, { recursive: true, force: true });
    }
    temporaryRoots.length = 0;
  });

  it("reports violations for forbidden imports", () => {
    const webRootDir = mkdtempSync(join(tmpdir(), "filament-boundary-check-"));
    temporaryRoots.push(webRootDir);

    writeFixture(webRootDir, "src/domain/auth.ts", 'import { createApi } from "../lib/api";\n');
    writeFixture(webRootDir, "src/lib/api.ts", 'import { panel } from "../features/app-shell/helpers";\n');
    writeFixture(webRootDir, "src/features/app-shell/helpers.ts", 'import { x } from "../../lib/api";\n');

    const result = runImportBoundaryCheck({ webRootDir });
    expect(result.scannedFiles).toBe(3);
    expect(result.violations).toEqual([
      {
        filePath: "src/domain/auth.ts",
        importSpecifier: "../lib/api",
        sourceLayer: "domain",
        targetLayer: "lib",
        reason: "domain must not import lib",
      },
      {
        filePath: "src/lib/api.ts",
        importSpecifier: "../features/app-shell/helpers",
        sourceLayer: "lib",
        targetLayer: "features",
        reason: "lib must not import features",
      },
    ]);
  });

  it("accepts allowed layer directions", () => {
    const webRootDir = mkdtempSync(join(tmpdir(), "filament-boundary-check-"));
    temporaryRoots.push(webRootDir);

    writeFixture(webRootDir, "src/domain/chat.ts", 'import { x } from "./auth";\n');
    writeFixture(webRootDir, "src/lib/gateway.ts", 'import { userIdFromInput } from "../domain/chat";\n');
    writeFixture(webRootDir, "src/features/app-shell/helpers.ts", 'import { ApiError } from "../../lib/api";\n');

    const result = runImportBoundaryCheck({ webRootDir });
    expect(result.scannedFiles).toBe(3);
    expect(result.violations).toEqual([]);
  });

  it("fails closed when source root escapes web root", () => {
    const webRootDir = mkdtempSync(join(tmpdir(), "filament-boundary-check-"));
    temporaryRoots.push(webRootDir);

    writeFixture(webRootDir, "src/domain/chat.ts", 'import { x } from "./auth";\n');

    expect(() =>
      runImportBoundaryCheck({
        webRootDir,
        sourceRoot: "../outside",
      }),
    ).toThrow("source root must stay within the web client root");
  });

  it("fails closed when source root is absolute", () => {
    const webRootDir = mkdtempSync(join(tmpdir(), "filament-boundary-check-"));
    temporaryRoots.push(webRootDir);

    writeFixture(webRootDir, "src/domain/chat.ts", 'import { x } from "./auth";\n');

    expect(() =>
      runImportBoundaryCheck({
        webRootDir,
        sourceRoot: "/tmp",
      }),
    ).toThrow("source root must be relative to the web client root");
  });

  it("fails closed when source file scan cap is exceeded", () => {
    const webRootDir = mkdtempSync(join(tmpdir(), "filament-boundary-check-"));
    temporaryRoots.push(webRootDir);

    writeFixture(webRootDir, "src/domain/auth.ts", 'import { x } from "./user";\n');
    writeFixture(webRootDir, "src/domain/user.ts", "export const user = true;\n");
    writeFixture(webRootDir, "src/lib/api.ts", "export const api = true;\n");

    expect(() =>
      runImportBoundaryCheck({
        webRootDir,
        maxScannedFiles: 2,
      }),
    ).toThrow("source file scan cap exceeded: 3 > 2");
  });
});
