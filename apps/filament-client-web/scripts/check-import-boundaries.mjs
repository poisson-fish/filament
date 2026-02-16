import { readdirSync, readFileSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { collectImportBoundaryViolations } from "./import-boundary-rules.mjs";

const DEFAULT_MODE = "warn";
const VALID_MODES = new Set(["warn", "enforce"]);
const SOURCE_ROOT = "src";
const SOURCE_FILE_EXTENSIONS = new Set([".ts", ".tsx", ".mts", ".cts"]);

function parseArgs(argv) {
  let modeArg = null;
  let sourceArg = null;

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
    if (arg.startsWith("--source=")) {
      sourceArg = arg.slice("--source=".length);
    }
  }

  return {
    modeArg,
    sourceArg,
  };
}

function resolveMode(modeArg) {
  const preferred = modeArg ?? process.env.FILAMENT_IMPORT_BOUNDARY_MODE;
  if (typeof preferred !== "string") {
    return DEFAULT_MODE;
  }
  const normalized = preferred.trim().toLowerCase();
  if (!VALID_MODES.has(normalized)) {
    return DEFAULT_MODE;
  }
  return normalized;
}

function collectSourceFilePaths(directoryPath) {
  const filePaths = [];
  const entries = readdirSync(directoryPath, { withFileTypes: true });

  for (const entry of entries) {
    const entryPath = join(directoryPath, entry.name);
    if (entry.isDirectory()) {
      filePaths.push(...collectSourceFilePaths(entryPath));
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    if (!SOURCE_FILE_EXTENSIONS.has(extname(entry.name))) {
      continue;
    }
    filePaths.push(entryPath);
  }

  return filePaths;
}

export function runImportBoundaryCheck({ webRootDir, sourceRoot = SOURCE_ROOT }) {
  const sourceDir = resolve(webRootDir, sourceRoot);
  const absolutePaths = collectSourceFilePaths(sourceDir);
  const files = absolutePaths.map((absolutePath) => ({
    path: relative(webRootDir, absolutePath),
    content: readFileSync(absolutePath, "utf8"),
  }));

  const violations = collectImportBoundaryViolations(files);
  return {
    scannedFiles: files.length,
    violations,
  };
}

function formatViolation(violation) {
  return [
    violation.filePath,
    `imports ${violation.importSpecifier}`,
    `(${violation.reason})`,
  ].join(" ");
}

function runCli() {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const webRootDir = resolve(scriptDir, "..");
  const args = parseArgs(process.argv.slice(2));
  const mode = resolveMode(args.modeArg);
  const sourceRoot =
    typeof args.sourceArg === "string" && args.sourceArg.trim().length > 0 ? args.sourceArg : SOURCE_ROOT;

  const { scannedFiles, violations } = runImportBoundaryCheck({
    webRootDir,
    sourceRoot,
  });

  if (violations.length === 0) {
    console.log(
      `[import-boundaries] OK: scanned ${scannedFiles} source files in ${sourceRoot} with no layer violations.`,
    );
    return;
  }

  for (const violation of violations) {
    console.error(`[import-boundaries] ${formatViolation(violation)}`);
  }

  const summary =
    `[import-boundaries] ${violations.length} violation(s) found while scanning ` +
    `${scannedFiles} source files in ${sourceRoot}.`;

  if (mode === "enforce") {
    console.error(`[import-boundaries] ERROR: ${summary}`);
    process.exitCode = 1;
    return;
  }

  console.warn(`[import-boundaries] WARNING: ${summary}`);
}

const isDirectExecution = process.argv[1] === fileURLToPath(import.meta.url);

if (isDirectExecution) {
  runCli();
}
