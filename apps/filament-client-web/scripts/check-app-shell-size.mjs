import { readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const STAGE_THRESHOLDS = Object.freeze({
  A: 1200,
  B: 1000,
  C: 850,
  D: 650,
});
const DEFAULT_STAGE = "D";
const DEFAULT_MODE = "warn";
const VALID_MODES = new Set(["warn", "enforce"]);

function parsePositiveInteger(rawValue) {
  if (typeof rawValue !== "string" || rawValue.trim().length === 0) {
    return null;
  }
  const parsed = Number.parseInt(rawValue, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }
  return parsed;
}

function parseArgs(argv) {
  let modeArg = null;
  let stageArg = null;
  let maxLinesArg = null;
  let fileArg = null;

  for (const arg of argv) {
    if (arg === "--enforce") {
      modeArg = "enforce";
      continue;
    }
    if (arg === "--warn") {
      modeArg = "warn";
      continue;
    }
    if (arg.startsWith("--mode=")) {
      modeArg = arg.slice("--mode=".length);
      continue;
    }
    if (arg.startsWith("--stage=")) {
      stageArg = arg.slice("--stage=".length);
      continue;
    }
    if (arg.startsWith("--max-lines=")) {
      maxLinesArg = arg.slice("--max-lines=".length);
      continue;
    }
    if (arg.startsWith("--file=")) {
      fileArg = arg.slice("--file=".length);
    }
  }

  return {
    modeArg,
    stageArg,
    maxLinesArg,
    fileArg,
  };
}

function resolveMode(cliModeArg) {
  const preferred = cliModeArg ?? process.env.FILAMENT_APP_SHELL_SIZE_MODE;
  if (typeof preferred !== "string") {
    return DEFAULT_MODE;
  }
  const normalized = preferred.trim().toLowerCase();
  if (!VALID_MODES.has(normalized)) {
    return DEFAULT_MODE;
  }
  return normalized;
}

function resolveStage(cliStageArg) {
  const preferred = cliStageArg ?? process.env.FILAMENT_APP_SHELL_SIZE_STAGE;
  if (typeof preferred !== "string") {
    return DEFAULT_STAGE;
  }
  const normalized = preferred.trim().toUpperCase();
  if (!(normalized in STAGE_THRESHOLDS)) {
    return DEFAULT_STAGE;
  }
  return normalized;
}

function resolveMaxLines({ cliMaxLinesArg, stage }) {
  const explicitMaxLines =
    parsePositiveInteger(cliMaxLinesArg) ??
    parsePositiveInteger(process.env.FILAMENT_APP_SHELL_MAX_LINES);
  if (explicitMaxLines !== null) {
    return {
      maxLines: explicitMaxLines,
      source: "explicit",
    };
  }
  return {
    maxLines: STAGE_THRESHOLDS[stage],
    source: "stage",
  };
}

function resolveAppShellPath({ scriptDir, cliFileArg }) {
  const preferredPath =
    (typeof cliFileArg === "string" && cliFileArg.trim().length > 0
      ? cliFileArg
      : null) ??
    process.env.FILAMENT_APP_SHELL_FILE ??
    "src/pages/AppShellPage.tsx";
  return resolve(resolve(scriptDir, ".."), preferredPath);
}

function countLines(sourceText) {
  if (sourceText.length === 0) {
    return 0;
  }
  const segments = sourceText.split(/\r?\n/);
  if (segments[segments.length - 1] === "") {
    segments.pop();
  }
  return segments.length;
}

const scriptDir = dirname(fileURLToPath(import.meta.url));
const args = parseArgs(process.argv.slice(2));
const mode = resolveMode(args.modeArg);
const stage = resolveStage(args.stageArg);
const { maxLines, source } = resolveMaxLines({
  cliMaxLinesArg: args.maxLinesArg,
  stage,
});
const webRootDir = resolve(scriptDir, "..");
const appShellPath = resolveAppShellPath({
  scriptDir,
  cliFileArg: args.fileArg,
});
const displayPath = relative(webRootDir, appShellPath);
const lineCount = countLines(readFileSync(appShellPath, "utf8"));
const thresholdContext =
  source === "stage"
    ? `stage ${stage}`
    : "explicit FILAMENT_APP_SHELL_MAX_LINES/--max-lines override";

if (lineCount > maxLines) {
  const breachMessage =
    `${displayPath} is ${lineCount} lines (threshold: ${maxLines}).` +
    ` Keep extracting controllers/components to reduce orchestration weight (${thresholdContext}).`;
  if (mode === "enforce") {
    console.error(`[size-check] ERROR: ${breachMessage}`);
    if (process.env.CI === "true") {
      console.log(`::error file=${displayPath}::${breachMessage}`);
    }
    process.exitCode = 1;
  } else {
    console.warn(`[size-check] WARNING: ${breachMessage}`);
    if (process.env.CI === "true") {
      console.log(`::warning file=${displayPath}::${breachMessage}`);
    }
  }
} else {
  console.log(
    `[size-check] OK: ${displayPath} is ${lineCount} lines (threshold: ${maxLines}, mode: ${mode}, ${thresholdContext}).`,
  );
}
