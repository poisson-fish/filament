#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

const MAX_WEB_BUNDLE_FILES = 4096;
const MAX_WEB_BUNDLE_BYTES = 128 * 1024 * 1024;
const MAX_WEB_ASSET_BYTES = 32 * 1024 * 1024;
const MAX_INDEX_HTML_BYTES = 1024 * 1024;
const MAX_ARTIFACT_BYTES = 1024 * 1024 * 1024;
const MIN_ARTIFACT_BYTES = 16;
const PACKAGE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const PLATFORM_FORMATS = Object.freeze({
  linux: Object.freeze([
    Object.freeze({ directory: "deb", kind: "deb", suffix: ".deb", type: "file" }),
    Object.freeze({
      directory: "appimage",
      kind: "appimage",
      suffix: ".AppImage",
      type: "file",
    }),
  ]),
  macos: Object.freeze([
    Object.freeze({ directory: "macos", kind: "app", suffix: ".app", type: "directory" }),
    Object.freeze({ directory: "dmg", kind: "dmg", suffix: ".dmg", type: "file" }),
  ]),
  windows: Object.freeze([
    Object.freeze({ directory: "msi", kind: "msi", suffix: ".msi", type: "file" }),
  ]),
  android: Object.freeze([
    Object.freeze({ directory: "apk", kind: "apk", suffix: ".apk", type: "file" }),
    Object.freeze({ directory: "bundle", kind: "aab", suffix: ".aab", type: "file" }),
  ]),
});

const FORBIDDEN_WEB_SUFFIXES = Object.freeze([
  ".env",
  ".key",
  ".map",
  ".mobileprovision",
  ".p12",
  ".pem",
  ".pfx",
]);

function fail(message) {
  throw new Error(`packaged-client verification failed: ${message}`);
}

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      fail("arguments must be supplied as --name value pairs");
    }
    if (values.has(name)) {
      fail(`duplicate argument ${name}`);
    }
    values.set(name, value);
  }

  const required = [
    "--platform",
    "--architecture",
    "--bundle-root",
    "--web-dist",
    "--manifest",
    "--checksums",
  ];
  for (const name of required) {
    if (!values.has(name)) {
      fail(`missing required argument ${name}`);
    }
  }

  return {
    platform: values.get("--platform"),
    architecture: values.get("--architecture"),
    bundleRoot: values.get("--bundle-root"),
    webDist: values.get("--web-dist"),
    manifestPath: values.get("--manifest"),
    checksumsPath: values.get("--checksums"),
  };
}

function portableRelative(root, target) {
  const relative = path.relative(root, target).split(path.sep).join("/");
  if (!relative || relative === ".." || relative.startsWith("../")) {
    fail("discovered path escaped its declared root");
  }
  return relative;
}

async function regularFiles(root, { includeDirectories = false } = {}) {
  const rootInfo = await lstat(root).catch(() => null);
  if (!rootInfo?.isDirectory() || rootInfo.isSymbolicLink()) {
    fail(`${root} must be a real directory`);
  }

  const files = [];
  const directories = [];
  const pending = [root];
  while (pending.length > 0) {
    const current = pending.pop();
    const entries = await readdir(current, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const absolute = path.join(current, entry.name);
      const info = await lstat(absolute);
      if (info.isSymbolicLink()) {
        fail(`symbolic links are forbidden: ${portableRelative(root, absolute)}`);
      }
      if (info.isDirectory()) {
        directories.push(absolute);
        pending.push(absolute);
      } else if (info.isFile()) {
        files.push(absolute);
      } else {
        fail(`non-regular bundle entry is forbidden: ${portableRelative(root, absolute)}`);
      }
    }
  }
  files.sort();
  directories.sort();
  return includeDirectories ? { files, directories } : files;
}

async function sha256File(filename) {
  const digest = createHash("sha256");
  for await (const chunk of createReadStream(filename)) {
    digest.update(chunk);
  }
  return digest.digest("hex");
}

function validateLocalAssetReference(value, field) {
  if (
    value.length === 0 ||
    value.startsWith("//") ||
    value.includes("\\") ||
    /[\u0000-\u001f\u007f]/u.test(value) ||
    /^[a-z][a-z0-9+.-]*:/iu.test(value)
  ) {
    fail(`${field} must reference a local bundled asset`);
  }
  const pathname = value.split(/[?#]/u, 1)[0];
  if (pathname.split("/").includes("..")) {
    fail(`${field} cannot traverse outside the local bundle`);
  }
}

function validateIndexHtml(raw) {
  for (const forbidden of ["<base", "<embed", "<iframe", "<object"]) {
    if (raw.toLowerCase().includes(forbidden)) {
      fail(`index.html contains forbidden element ${forbidden}`);
    }
  }

  const scripts = [...raw.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script\s*>/giu)];
  if (scripts.length === 0) {
    fail("index.html must load a local application script");
  }
  for (const script of scripts) {
    const source = script[1].match(/\bsrc\s*=\s*["']([^"']+)["']/iu)?.[1];
    if (!source || script[2].trim().length > 0) {
      fail("inline or source-less scripts are forbidden");
    }
    validateLocalAssetReference(source, "script src");
  }

  for (const link of raw.matchAll(/<link\b([^>]*)>/giu)) {
    const href = link[1].match(/\bhref\s*=\s*["']([^"']+)["']/iu)?.[1];
    if (!href) {
      fail("link elements must have a local href");
    }
    validateLocalAssetReference(href, "link href");
  }
}

async function verifyWebBundle(webDist) {
  const files = await regularFiles(webDist);
  if (files.length === 0 || files.length > MAX_WEB_BUNDLE_FILES) {
    fail("web bundle file count is outside the hard limit");
  }

  const records = [];
  let totalBytes = 0;
  for (const filename of files) {
    const relativePath = portableRelative(webDist, filename);
    const lowered = relativePath.toLowerCase();
    if (FORBIDDEN_WEB_SUFFIXES.some((suffix) => lowered.endsWith(suffix))) {
      fail(`forbidden web bundle file: ${relativePath}`);
    }
    const info = await stat(filename);
    if (info.size > MAX_WEB_ASSET_BYTES) {
      fail(`web asset exceeds the per-file hard limit: ${relativePath}`);
    }
    totalBytes += info.size;
    if (totalBytes > MAX_WEB_BUNDLE_BYTES) {
      fail("web bundle exceeds the aggregate hard limit");
    }
    records.push({
      path: relativePath,
      bytes: info.size,
      sha256: await sha256File(filename),
    });
  }

  const index = records.find((record) => record.path === "index.html");
  if (!index) {
    fail("web bundle is missing index.html");
  }
  if (index.bytes > MAX_INDEX_HTML_BYTES) {
    fail("index.html exceeds its parsing hard limit");
  }
  validateIndexHtml(await readFile(path.join(webDist, "index.html"), "utf8"));

  const digest = createHash("sha256");
  for (const record of records) {
    digest.update(record.path);
    digest.update("\0");
    digest.update(record.sha256);
    digest.update("\n");
  }
  return {
    file_count: records.length,
    bytes: totalBytes,
    sha256: digest.digest("hex"),
  };
}

async function directoryArtifactRecord(bundleRoot, artifact, kind) {
  const files = await regularFiles(artifact);
  if (files.length === 0) {
    fail(`artifact directory is empty: ${portableRelative(bundleRoot, artifact)}`);
  }
  const relativeArtifact = portableRelative(bundleRoot, artifact);
  if (kind === "app") {
    const relativeFiles = files.map((filename) => portableRelative(artifact, filename));
    if (!relativeFiles.includes("Contents/Resources/THIRD_PARTY_NOTICES.txt")) {
      fail("macOS app is missing THIRD_PARTY_NOTICES.txt");
    }
    if (!relativeFiles.some((filename) => filename.startsWith("Contents/MacOS/"))) {
      fail("macOS app is missing its native executable");
    }
  }

  let bytes = 0;
  const digest = createHash("sha256");
  for (const filename of files) {
    const relative = portableRelative(artifact, filename);
    const info = await stat(filename);
    bytes += info.size;
    if (bytes > MAX_ARTIFACT_BYTES) {
      fail(`artifact exceeds the hard limit: ${relativeArtifact}`);
    }
    digest.update(relative);
    digest.update("\0");
    digest.update(await sha256File(filename));
    digest.update("\n");
  }
  return {
    path: `${relativeArtifact}/`,
    kind,
    bytes,
    sha256: digest.digest("hex"),
  };
}

async function fileArtifactRecord(bundleRoot, artifact, kind) {
  const info = await stat(artifact);
  if (info.size < MIN_ARTIFACT_BYTES || info.size > MAX_ARTIFACT_BYTES) {
    fail(`artifact size is outside the hard limit: ${portableRelative(bundleRoot, artifact)}`);
  }
  return {
    path: portableRelative(bundleRoot, artifact),
    kind,
    bytes: info.size,
    sha256: await sha256File(artifact),
  };
}

async function verifyArtifacts(platform, bundleRoot) {
  const expected = PLATFORM_FORMATS[platform];
  if (!expected) {
    fail(`unsupported platform ${platform}`);
  }
  const { files, directories } = await regularFiles(bundleRoot, { includeDirectories: true });
  const records = [];
  for (const format of expected) {
    const candidates = (format.type === "file" ? files : directories).filter((candidate) => {
      const relative = portableRelative(bundleRoot, candidate);
      return relative.startsWith(`${format.directory}/`) && candidate.endsWith(format.suffix);
    });
    if (candidates.length !== 1) {
      fail(`expected exactly one ${format.kind} artifact, found ${candidates.length}`);
    }
    if (
      platform === "android" &&
      (!path.basename(candidates[0]).toLowerCase().includes("release") ||
        path.basename(candidates[0]).toLowerCase().includes("debug"))
    ) {
      fail(`Android ${format.kind} must be a release-variant artifact`);
    }
    records.push(
      format.type === "file"
        ? await fileArtifactRecord(bundleRoot, candidates[0], format.kind)
        : await directoryArtifactRecord(bundleRoot, candidates[0], format.kind),
    );
  }
  return records;
}

async function verifyPackagingContracts(platform, architecture) {
  const support = JSON.parse(
    await readFile(path.join(PACKAGE_ROOT, "platform-support.json"), "utf8"),
  );
  if (
    support?.support_policy?.remote_application_code_allowed !== false ||
    support?.support_policy?.automatic_updates_enabled !== false
  ) {
    fail("platform support contract must forbid remote code and automatic updates");
  }
  const target = support.targets?.find((candidate) => candidate.target === platform);
  if (!target || !target.architectures?.includes(architecture)) {
    fail("platform and architecture must match the reviewed support contract");
  }
  const expectedFormats = PLATFORM_FORMATS[platform]?.map((format) => format.kind);
  if (JSON.stringify(target.package_formats) !== JSON.stringify(expectedFormats)) {
    fail("package formats must exactly match the reviewed support contract");
  }

  const tauriConfig = JSON.parse(
    await readFile(path.join(PACKAGE_ROOT, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  if (tauriConfig?.bundle?.createUpdaterArtifacts !== false) {
    fail("Tauri update artifacts must remain disabled");
  }
  if (platform === "android" && tauriConfig?.bundle?.android?.minSdkVersion !== 33) {
    fail("Android minimum SDK must remain API 33");
  }
  return {
    remoteApplicationCodeAllowed: support.support_policy.remote_application_code_allowed,
    updateArtifactsEnabled: tauriConfig.bundle.createUpdaterArtifacts,
  };
}

export async function verifyPackage(options) {
  if (!/^[a-z0-9_]+$/u.test(options.architecture)) {
    fail("architecture must be a closed machine identifier");
  }
  const invocationRoot = path.resolve(options.invocationRoot ?? process.env.INIT_CWD ?? process.cwd());
  const bundleRoot = path.resolve(invocationRoot, options.bundleRoot);
  const webDist = path.resolve(invocationRoot, options.webDist);
  const manifestPath = path.resolve(invocationRoot, options.manifestPath);
  const checksumsPath = path.resolve(invocationRoot, options.checksumsPath);
  const contracts = await verifyPackagingContracts(options.platform, options.architecture);
  const artifacts = await verifyArtifacts(options.platform, bundleRoot);
  const webBundle = await verifyWebBundle(webDist);
  const manifest = {
    schema_version: 1,
    platform: options.platform,
    architecture: options.architecture,
    update_artifacts_enabled: contracts.updateArtifactsEnabled,
    remote_application_code_allowed: contracts.remoteApplicationCodeAllowed,
    artifacts,
    web_bundle: webBundle,
  };

  await mkdir(path.dirname(manifestPath), { recursive: true });
  await mkdir(path.dirname(checksumsPath), { recursive: true });
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, {
    mode: 0o600,
  });
  const checksums = artifacts
    .map((artifact) => `${artifact.sha256}  ${artifact.path}`)
    .join("\n");
  await writeFile(checksumsPath, `${checksums}\n`, { mode: 0o600 });
  return manifest;
}

const invokedDirectly =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (invokedDirectly) {
  verifyPackage(parseArguments(process.argv.slice(2))).catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : "verification failed"}\n`);
    process.exitCode = 1;
  });
}
