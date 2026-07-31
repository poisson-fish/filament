import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { runMobilePackageSmoke } from "../tools/smoke-mobile-package.mjs";

const temporaryRoots = [];
const IOS_DEVICE = "11111111-2222-3333-4444-555555555555";

test.afterEach(async () => {
  await Promise.all(temporaryRoots.splice(0).map((root) => rm(root, { recursive: true })));
});

async function fixture(platform) {
  const root = await mkdtemp(path.join(os.tmpdir(), "filament-mobile-smoke-"));
  temporaryRoots.push(root);
  const packagePath =
    platform === "android"
      ? path.join(root, "filament.apk")
      : path.join(root, "Filament.app");
  if (platform === "android") {
    await writeFile(packagePath, "bounded APK fixture");
  } else {
    await mkdir(packagePath);
  }
  return {
    packagePath,
    evidencePath: path.join(root, "evidence", `${platform}-launch.json`),
  };
}

function androidRunner({ crash = false, processChanges = false } = {}) {
  let pidReads = 0;
  return async (file, args) => {
    assert.equal(file, "adb");
    const command = args.slice(2);
    if (command[0] === "get-state") {
      return { stdout: "device\n", stderr: "" };
    }
    if (command.join(" ") === "shell settings get global airplane_mode_on") {
      return { stdout: "1\n", stderr: "" };
    }
    if (command.includes("resolve-activity")) {
      return { stdout: "com.filament.desktop/.MainActivity\n", stderr: "" };
    }
    if (command.includes("pidof")) {
      pidReads += 1;
      const pid = processChanges && pidReads === 2 ? "42" : "41";
      return { stdout: `${pid}\n`, stderr: "" };
    }
    if (command[0] === "logcat" && command[1] === "-d") {
      return {
        stdout: crash ? "FATAL EXCEPTION: main\nsecret server response" : "",
        stderr: "",
      };
    }
    return { stdout: "Success\n", stderr: "" };
  };
}

function iosRunner() {
  let nextPid = 100;
  return async (file, args) => {
    assert.equal(file, "xcrun");
    if (args[1] === "launch") {
      nextPid += 1;
      return {
        stdout: `com.filament.desktop: ${nextPid}\n`,
        stderr: "",
      };
    }
    return { stdout: "", stderr: "" };
  };
}

test("records redacted Android install restart and reinstall evidence", async () => {
  const options = await fixture("android");
  const evidence = await runMobilePackageSmoke({
    platform: "android",
    ...options,
    device: "emulator-5554",
    observationMs: 250,
    commandRunner: androidRunner(),
    delayRunner: async () => {},
  });

  assert.deepEqual(evidence, {
    schema_version: 1,
    platform: "android",
    package: "filament.apk",
    application_identifier: "com.filament.desktop",
    observation_ms: 250,
    installed_launch: true,
    restart_launch: true,
    uninstall_reinstall_launch: true,
    native_startup_fail_closed: true,
    offline_bundle_launch: true,
  });
  assert.deepEqual(JSON.parse(await readFile(options.evidencePath, "utf8")), evidence);
  assert.doesNotMatch(JSON.stringify(evidence), /emulator-5554/u);
});

test("rejects Android process replacement and crash output without echoing logs", async () => {
  const changed = await fixture("android");
  await assert.rejects(
    runMobilePackageSmoke({
      platform: "android",
      ...changed,
      device: "emulator-5554",
      observationMs: 250,
      commandRunner: androidRunner({ processChanges: true }),
      delayRunner: async () => {},
    }),
    /restarted during observation/u,
  );

  const crashed = await fixture("android");
  await assert.rejects(
    runMobilePackageSmoke({
      platform: "android",
      ...crashed,
      device: "emulator-5554",
      observationMs: 250,
      commandRunner: androidRunner({ crash: true }),
      delayRunner: async () => {},
    }),
    (error) => {
      assert.match(error.message, /native startup failure/u);
      assert.doesNotMatch(error.message, /secret server response/u);
      return true;
    },
  );
});

test("records redacted iOS install restart and reinstall evidence", async () => {
  const options = await fixture("ios");
  const alive = new Set([101, 102, 103]);
  let finalPidChecks = 0;
  const evidence = await runMobilePackageSmoke({
    platform: "ios",
    ...options,
    device: IOS_DEVICE,
    observationMs: 250,
    commandRunner: iosRunner(),
    delayRunner: async () => {},
    processAlive: (pid) => {
      if (pid === 103) {
        finalPidChecks += 1;
        if (finalPidChecks === 2) {
          alive.delete(pid);
        }
      }
      return alive.has(pid);
    },
  });

  assert.equal(evidence.platform, "ios");
  assert.equal(evidence.package, "Filament.app");
  assert.equal(evidence.uninstall_reinstall_launch, true);
  assert.doesNotMatch(JSON.stringify(evidence), /11111111/u);
});

test("rejects an iOS package that exits during observation", async () => {
  const options = await fixture("ios");
  await assert.rejects(
    runMobilePackageSmoke({
      platform: "ios",
      ...options,
      device: IOS_DEVICE,
      observationMs: 250,
      commandRunner: iosRunner(),
      delayRunner: async () => {},
      processAlive: () => false,
    }),
    /exited during observation/u,
  );
});
