import assert from "node:assert/strict";
import { mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { runDesktopPackageSmoke } from "../tools/smoke-desktop-package.mjs";

const temporaryRoots = [];
const platform =
  process.platform === "darwin"
    ? "macos"
    : process.platform === "win32"
      ? "windows"
      : "linux";

test.afterEach(async () => {
  await Promise.all(temporaryRoots.splice(0).map((root) => rm(root, { recursive: true })));
});

async function fixture(script) {
  const root = await mkdtemp(path.join(os.tmpdir(), "filament-desktop-smoke-"));
  temporaryRoots.push(root);
  const scriptPath = path.join(root, "fixture.mjs");
  const evidencePath = path.join(root, "evidence", "launch.json");
  await writeFile(scriptPath, script);
  return {
    executable: await realpath(process.execPath),
    executableArgs: [scriptPath],
    evidencePath,
  };
}

test("records bounded evidence for a living offline process", async () => {
  const options = await fixture("setInterval(() => {}, 10_000);\n");
  const evidence = await runDesktopPackageSmoke({
    platform,
    ...options,
    observationMs: 750,
  });

  assert.deepEqual(evidence, {
    schema_version: 1,
    platform,
    executable: path.basename(options.executable),
    observation_ms: 750,
    process_alive: true,
    network_socket_count: 0,
    stdout_bytes: 0,
    stderr_bytes: 0,
    offline_bundle_launch: true,
  });
  assert.deepEqual(JSON.parse(await readFile(options.evidencePath, "utf8")), evidence);
});

test("rejects a package process that exits during startup", async () => {
  const options = await fixture("process.exit(0);\n");
  await assert.rejects(
    runDesktopPackageSmoke({
      platform,
      ...options,
      observationMs: 750,
    }),
    /exited before the offline launch observation completed/u,
  );
});

test("rejects a package process that opens a network socket", async () => {
  const options = await fixture(`
    import net from "node:net";
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      net.connect(server.address().port, "127.0.0.1");
    });
    setInterval(() => {}, 10_000);
  `);
  await assert.rejects(
    runDesktopPackageSmoke({
      platform,
      ...options,
      observationMs: 1_500,
    }),
    /opened a network socket during offline launch/u,
  );
});

test("rejects output floods without including process output in the error", async () => {
  const options = await fixture(
    'process.stdout.write("sensitive-value".repeat(8_192)); setInterval(() => {}, 10_000);\n',
  );
  await assert.rejects(
    runDesktopPackageSmoke({
      platform,
      ...options,
      observationMs: 750,
    }),
    (error) => {
      assert.match(error.message, /bounded capture limit/u);
      assert.doesNotMatch(error.message, /sensitive-value/u);
      return true;
    },
  );
});
