import { readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_MAX_LINES = 1400;

function resolveMaxLines() {
  const rawValue = process.env.FILAMENT_APP_SHELL_MAX_LINES;
  if (typeof rawValue !== "string" || rawValue.trim().length === 0) {
    return DEFAULT_MAX_LINES;
  }

  const parsed = Number.parseInt(rawValue, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return DEFAULT_MAX_LINES;
  }
  return parsed;
}

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webRootDir = resolve(scriptDir, "..");
const appShellPath = resolve(webRootDir, "src/pages/AppShellPage.tsx");
const displayPath = relative(webRootDir, appShellPath);
const maxLines = resolveMaxLines();
const lineCount = readFileSync(appShellPath, "utf8").split(/\r?\n/).length;

if (lineCount > maxLines) {
  const warningMessage =
    `${displayPath} is ${lineCount} lines (threshold: ${maxLines}).` +
    " Keep extracting controllers/components to reduce orchestration weight.";
  console.warn(`[size-check] WARNING: ${warningMessage}`);
  if (process.env.CI === "true") {
    console.log(`::warning file=${displayPath}::${warningMessage}`);
  }
} else {
  console.log(
    `[size-check] OK: ${displayPath} is ${lineCount} lines (threshold: ${maxLines}).`,
  );
}
