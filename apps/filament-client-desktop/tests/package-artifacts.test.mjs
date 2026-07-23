import assert from "node:assert/strict";
import { chmod, mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { verifyPackage } from "../tools/verify-package.mjs";

const temporaryRoots = [];

test.afterEach(async () => {
  await Promise.all(temporaryRoots.splice(0).map((root) => rm(root, { recursive: true })));
});

async function fixture() {
  const root = await mkdtemp(path.join(os.tmpdir(), "filament-package-policy-"));
  temporaryRoots.push(root);
  const bundleRoot = path.join(root, "bundle");
  const webDist = path.join(root, "dist");
  await mkdir(bundleRoot);
  await mkdir(path.join(webDist, "assets"), { recursive: true });
  await writeFile(
    path.join(webDist, "index.html"),
    '<!doctype html><script type="module" src="/assets/app.js"></script><link rel="stylesheet" href="/assets/app.css"><div id="root"></div>',
  );
  await writeFile(path.join(webDist, "assets", "app.js"), "export {};\n");
  await writeFile(path.join(webDist, "assets", "app.css"), ":root { color: black; }\n");
  return {
    root,
    bundleRoot,
    webDist,
    manifestPath: path.join(root, "evidence", "manifest.json"),
    checksumsPath: path.join(root, "evidence", "SHA256SUMS"),
  };
}

test("accepts exact Linux formats and emits deterministic evidence", async () => {
  const options = await fixture();
  await mkdir(path.join(options.bundleRoot, "deb"));
  await mkdir(path.join(options.bundleRoot, "appimage"));
  await writeFile(path.join(options.bundleRoot, "deb", "Filament.deb"), "d".repeat(32));
  await writeFile(
    path.join(options.bundleRoot, "appimage", "Filament.AppImage"),
    "a".repeat(32),
  );

  const manifest = await verifyPackage({
    ...options,
    platform: "linux",
    architecture: "x86_64",
  });

  assert.equal(manifest.schema_version, 1);
  assert.deepEqual(
    manifest.artifacts.map((artifact) => artifact.kind),
    ["deb", "appimage"],
  );
  assert.equal(manifest.remote_application_code_allowed, false);
  assert.equal(manifest.web_bundle.file_count, 3);
  const checksums = await readFile(options.checksumsPath, "utf8");
  assert.match(checksums, /^[a-f0-9]{64}  deb\/Filament\.deb$/mu);
  assert.match(checksums, /^[a-f0-9]{64}  appimage\/Filament\.AppImage$/mu);
});

test("accepts a macOS app only when notices and a native executable are present", async () => {
  const options = await fixture();
  const app = path.join(options.bundleRoot, "macos", "Filament.app", "Contents");
  await mkdir(path.join(app, "MacOS"), { recursive: true });
  await mkdir(path.join(app, "Resources"), { recursive: true });
  await mkdir(path.join(options.bundleRoot, "dmg"), { recursive: true });
  await writeFile(path.join(app, "MacOS", "filament"), "binary");
  await writeFile(path.join(app, "Resources", "THIRD_PARTY_NOTICES.txt"), "notices");
  await writeFile(path.join(options.bundleRoot, "dmg", "Filament.dmg"), "d".repeat(32));

  const manifest = await verifyPackage({
    ...options,
    platform: "macos",
    architecture: "aarch64",
  });
  assert.deepEqual(
    manifest.artifacts.map((artifact) => artifact.kind),
    ["app", "dmg"],
  );
});

test("accepts exact Android APK and AAB formats for the reviewed arm64 target", async () => {
  const options = await fixture();
  await mkdir(path.join(options.bundleRoot, "apk", "universal", "release"), {
    recursive: true,
  });
  await mkdir(path.join(options.bundleRoot, "bundle", "universalRelease"), {
    recursive: true,
  });
  await writeFile(
    path.join(
      options.bundleRoot,
      "apk",
      "universal",
      "release",
      "app-universal-release-unsigned.apk",
    ),
    "p".repeat(32),
  );
  await writeFile(
    path.join(
      options.bundleRoot,
      "bundle",
      "universalRelease",
      "app-universal-release.aab",
    ),
    "b".repeat(32),
  );

  const manifest = await verifyPackage({
    ...options,
    platform: "android",
    architecture: "aarch64",
  });

  assert.deepEqual(
    manifest.artifacts.map((artifact) => artifact.kind),
    ["apk", "aab"],
  );
  const checksums = await readFile(options.checksumsPath, "utf8");
  assert.match(checksums, /^[a-f0-9]{64}  apk\/.+\.apk$/mu);
  assert.match(checksums, /^[a-f0-9]{64}  bundle\/.+\.aab$/mu);
});

test("accepts an unsigned iOS simulator app without requiring a device IPA", async () => {
  const options = await fixture();
  const app = path.join(options.bundleRoot, "aarch64-sim", "Filament.app");
  await mkdir(app, { recursive: true });
  await writeFile(path.join(app, "Filament"), "iOS simulator executable");
  await chmod(path.join(app, "Filament"), 0o700);
  await writeFile(path.join(app, "Info.plist"), "fixture plist");
  await writeFile(path.join(app, "THIRD_PARTY_NOTICES.txt"), "notices");

  const manifest = await verifyPackage({
    ...options,
    platform: "ios",
    architecture: "aarch64_simulator",
  });

  assert.deepEqual(
    manifest.artifacts.map((artifact) => artifact.kind),
    ["app"],
  );
  const checksums = await readFile(options.checksumsPath, "utf8");
  assert.match(checksums, /^[a-f0-9]{64}  aarch64-sim\/Filament\.app\/$/mu);
});

test("requires both an iOS device app and IPA for signed release evidence", async () => {
  const options = await fixture();
  const app = path.join(options.bundleRoot, "arm64", "Filament.app");
  await mkdir(app, { recursive: true });
  await writeFile(path.join(app, "Filament"), "iOS device executable");
  await chmod(path.join(app, "Filament"), 0o700);
  await writeFile(path.join(app, "Info.plist"), "fixture plist");
  await writeFile(path.join(app, "THIRD_PARTY_NOTICES.txt"), "notices");

  await assert.rejects(
    verifyPackage({ ...options, platform: "ios", architecture: "aarch64" }),
    /expected exactly one ipa artifact, found 0/u,
  );

  await writeFile(path.join(options.bundleRoot, "arm64", "Filament.ipa"), "p".repeat(32));
  const manifest = await verifyPackage({
    ...options,
    platform: "ios",
    architecture: "aarch64",
  });
  assert.deepEqual(
    manifest.artifacts.map((artifact) => artifact.kind),
    ["app", "ipa"],
  );
});

test("rejects a remote application script before writing evidence", async () => {
  const options = await fixture();
  await mkdir(path.join(options.bundleRoot, "msi"));
  await writeFile(path.join(options.bundleRoot, "msi", "Filament.msi"), "m".repeat(32));
  await writeFile(
    path.join(options.webDist, "index.html"),
    '<script src="https://hostile.example/app.js"></script>',
  );

  await assert.rejects(
    verifyPackage({ ...options, platform: "windows", architecture: "x86_64" }),
    /script src must reference a local bundled asset/u,
  );
});

test("rejects Android debug packages from release artifact evidence", async () => {
  const options = await fixture();
  await mkdir(path.join(options.bundleRoot, "apk"), { recursive: true });
  await mkdir(path.join(options.bundleRoot, "bundle"), { recursive: true });
  await writeFile(
    path.join(options.bundleRoot, "apk", "app-universal-debug.apk"),
    "p".repeat(32),
  );
  await writeFile(
    path.join(options.bundleRoot, "bundle", "app-universal-debug.aab"),
    "b".repeat(32),
  );

  await assert.rejects(
    verifyPackage({ ...options, platform: "android", architecture: "aarch64" }),
    /Android apk must be a release-variant artifact/u,
  );
});

test("rejects missing or duplicate required package formats", async () => {
  const options = await fixture();
  await mkdir(path.join(options.bundleRoot, "deb"));
  await writeFile(path.join(options.bundleRoot, "deb", "first.deb"), "1".repeat(32));
  await writeFile(path.join(options.bundleRoot, "deb", "second.deb"), "2".repeat(32));

  await assert.rejects(
    verifyPackage({ ...options, platform: "linux", architecture: "x86_64" }),
    /expected exactly one deb artifact, found 2/u,
  );
});

test("rejects source maps from the signed local application bundle", async () => {
  const options = await fixture();
  await mkdir(path.join(options.bundleRoot, "msi"));
  await writeFile(path.join(options.bundleRoot, "msi", "Filament.msi"), "m".repeat(32));
  await writeFile(path.join(options.webDist, "assets", "app.js.map"), "{}");

  await assert.rejects(
    verifyPackage({ ...options, platform: "windows", architecture: "x86_64" }),
    /forbidden web bundle file: assets\/app\.js\.map/u,
  );
});

test(
  "rejects symbolic links anywhere below the artifact root",
  { skip: process.platform === "win32" },
  async () => {
    const options = await fixture();
    await mkdir(path.join(options.bundleRoot, "msi"));
    const artifact = path.join(options.bundleRoot, "msi", "Filament.msi");
    await writeFile(artifact, "m".repeat(32));
    await symlink(artifact, path.join(options.bundleRoot, "unexpected-link"));

    await assert.rejects(
      verifyPackage({ ...options, platform: "windows", architecture: "x86_64" }),
      /symbolic links are forbidden/u,
    );
  },
);
