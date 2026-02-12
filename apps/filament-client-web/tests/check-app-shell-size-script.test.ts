// @vitest-environment node
// @ts-nocheck

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

const testDir = resolve(fileURLToPath(new URL(".", import.meta.url)));
const webRootDir = resolve(testDir, "..");
const scriptPath = resolve(webRootDir, "scripts/check-app-shell-size.mjs");
const tempDirs: string[] = [];

function createFixtureAppShell(lineCount: number): string {
  const fixtureDir = mkdtempSync(join(tmpdir(), "filament-app-shell-size-"));
  tempDirs.push(fixtureDir);
  const filePath = join(fixtureDir, "AppShellPage.tsx");
  const lines = Array.from({ length: lineCount }, (_, index) => `line-${index + 1}`);
  writeFileSync(filePath, lines.join("\n"), "utf8");
  return filePath;
}

function runSizeScript(options?: {
  args?: string[];
  env?: Record<string, string | undefined>;
}) {
  return spawnSync(process.execPath, [scriptPath, ...(options?.args ?? [])], {
    cwd: webRootDir,
    encoding: "utf8",
    env: {
      ...process.env,
      ...options?.env,
    },
  });
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const nextDir = tempDirs.pop();
    if (nextDir) {
      rmSync(nextDir, { recursive: true, force: true });
    }
  }
});

describe("check-app-shell-size script", () => {
  it("uses warning mode by default with stage D threshold", () => {
    const fixturePath = createFixtureAppShell(700);
    const result = runSizeScript({
      env: {
        FILAMENT_APP_SHELL_FILE: fixturePath,
      },
    });

    const output = `${result.stdout}\n${result.stderr}`;
    expect(result.status).toBe(0);
    expect(output).toContain("[size-check] WARNING:");
    expect(output).toContain("threshold: 650");
    expect(output).toContain("stage D");
  });

  it("fails in enforce mode when threshold is exceeded", () => {
    const fixturePath = createFixtureAppShell(900);
    const result = runSizeScript({
      env: {
        FILAMENT_APP_SHELL_FILE: fixturePath,
        FILAMENT_APP_SHELL_SIZE_MODE: "enforce",
        FILAMENT_APP_SHELL_SIZE_STAGE: "C",
      },
    });

    const output = `${result.stdout}\n${result.stderr}`;
    expect(result.status).toBe(1);
    expect(output).toContain("[size-check] ERROR:");
    expect(output).toContain("threshold: 850");
    expect(output).toContain("stage C");
  });

  it("lets explicit max-lines override stage thresholds", () => {
    const fixturePath = createFixtureAppShell(325);
    const result = runSizeScript({
      env: {
        FILAMENT_APP_SHELL_FILE: fixturePath,
        FILAMENT_APP_SHELL_MAX_LINES: "300",
        FILAMENT_APP_SHELL_SIZE_STAGE: "A",
      },
    });

    const output = `${result.stdout}\n${result.stderr}`;
    expect(result.status).toBe(0);
    expect(output).toContain("[size-check] WARNING:");
    expect(output).toContain("threshold: 300");
    expect(output).toContain("explicit FILAMENT_APP_SHELL_MAX_LINES/--max-lines override");
  });

  it("passes in enforce mode when below threshold", () => {
    const fixturePath = createFixtureAppShell(600);
    const result = runSizeScript({
      args: ["--enforce", "--stage=D"],
      env: {
        FILAMENT_APP_SHELL_FILE: fixturePath,
      },
    });

    const output = `${result.stdout}\n${result.stderr}`;
    expect(result.status).toBe(0);
    expect(output).toContain("[size-check] OK:");
    expect(output).toContain("threshold: 650");
    expect(output).toContain("mode: enforce");
  });
});
