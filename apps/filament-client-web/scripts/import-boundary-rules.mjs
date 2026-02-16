import { dirname, posix, sep } from "node:path";

const DOMAIN_LAYER = "domain";
const LIB_LAYER = "lib";
const FEATURES_LAYER = "features";

const SOURCE_ROOT = "src";
const LAYERS = new Set([DOMAIN_LAYER, LIB_LAYER, FEATURES_LAYER]);
const IMPORT_STATEMENT_PATTERN = /(?:import|export)\s+(?:[^"']*?\s+from\s*)?["']([^"']+)["']/g;

function toPosixPath(filePath) {
  return filePath.split(sep).join("/");
}

function stripLeadingDotSegments(path) {
  return path.replace(/^\.\/+/, "");
}

function readLayerFromProjectPath(projectPath) {
  const normalized = stripLeadingDotSegments(toPosixPath(projectPath));
  if (!normalized.startsWith(`${SOURCE_ROOT}/`)) {
    return null;
  }
  const [, layer] = normalized.split("/");
  if (!LAYERS.has(layer)) {
    return null;
  }
  return layer;
}

export function classifySourceLayer(projectPath) {
  return readLayerFromProjectPath(projectPath);
}

export function extractImportSpecifiers(sourceText) {
  const specifiers = [];
  for (const match of sourceText.matchAll(IMPORT_STATEMENT_PATTERN)) {
    const [, specifier] = match;
    if (typeof specifier === "string" && specifier.length > 0) {
      specifiers.push(specifier);
    }
  }
  return specifiers;
}

export function resolveProjectImportTarget({ projectPath, importSpecifier }) {
  if (typeof importSpecifier !== "string" || importSpecifier.length === 0) {
    return null;
  }

  if (importSpecifier.startsWith("./") || importSpecifier.startsWith("../")) {
    const fromDirectory = dirname(projectPath);
    const resolved = posix.normalize(posix.join(toPosixPath(fromDirectory), importSpecifier));
    return stripLeadingDotSegments(resolved);
  }

  if (importSpecifier.startsWith("src/")) {
    return stripLeadingDotSegments(importSpecifier);
  }

  if (importSpecifier.startsWith("/src/")) {
    return stripLeadingDotSegments(importSpecifier.slice(1));
  }

  return null;
}

function isForbiddenBoundary(sourceLayer, targetLayer) {
  if (sourceLayer === DOMAIN_LAYER && (targetLayer === LIB_LAYER || targetLayer === FEATURES_LAYER)) {
    return true;
  }
  if (sourceLayer === LIB_LAYER && targetLayer === FEATURES_LAYER) {
    return true;
  }
  return false;
}

function violationReason(sourceLayer, targetLayer) {
  return `${sourceLayer} must not import ${targetLayer}`;
}

export function collectImportBoundaryViolations(files) {
  const violations = [];

  for (const file of files) {
    const sourceLayer = classifySourceLayer(file.path);
    if (sourceLayer === null) {
      continue;
    }

    const specifiers = extractImportSpecifiers(file.content);
    for (const importSpecifier of specifiers) {
      const projectTargetPath = resolveProjectImportTarget({
        projectPath: file.path,
        importSpecifier,
      });
      if (projectTargetPath === null) {
        continue;
      }

      const targetLayer = classifySourceLayer(projectTargetPath);
      if (targetLayer === null) {
        continue;
      }

      if (!isForbiddenBoundary(sourceLayer, targetLayer)) {
        continue;
      }

      violations.push({
        filePath: file.path,
        importSpecifier,
        sourceLayer,
        targetLayer,
        reason: violationReason(sourceLayer, targetLayer),
      });
    }
  }

  return violations;
}
