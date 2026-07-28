#!/usr/bin/env node

import { execFile } from "node:child_process";
import { lstat, mkdir, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const APP_IDENTIFIER = "com.filament.desktop";
const MAX_CAPTURE_BYTES = 64 * 1024;
const MIN_OBSERVATION_MS = 250;
const MAX_OBSERVATION_MS = 30_000;
const DEFAULT_OBSERVATION_MS = 8_000;
const COMMAND_TIMEOUT_MS = 30_000;

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) {
      throw new Error("arguments must be provided as --name value pairs");
    }
    if (Object.hasOwn(options, flag)) {
      throw new Error(`duplicate argument: ${flag}`);
    }
    options[flag] = value;
  }

  const allowed = new Set([
    "--platform",
    "--package",
    "--device",
    "--evidence",
    "--observation-ms",
  ]);
  for (const flag of Object.keys(options)) {
    if (!allowed.has(flag)) {
      throw new Error(`unknown argument: ${flag}`);
    }
  }
  for (const required of ["--platform", "--package", "--device", "--evidence"]) {
    if (!options[required]) {
      throw new Error(`missing required argument: ${required}`);
    }
  }

  const observationValue = options["--observation-ms"];
  if (observationValue !== undefined && !/^\d+$/u.test(observationValue)) {
    throw new Error("observation time must be an integer");
  }
  const observationMs =
    observationValue === undefined
      ? DEFAULT_OBSERVATION_MS
      : Number.parseInt(observationValue, 10);
  validateObservationTime(observationMs);

  return {
    platform: options["--platform"],
    packagePath: path.resolve(options["--package"]),
    device: options["--device"],
    evidencePath: path.resolve(options["--evidence"]),
    observationMs,
  };
}

function validateObservationTime(observationMs) {
  if (
    !Number.isSafeInteger(observationMs) ||
    observationMs < MIN_OBSERVATION_MS ||
    observationMs > MAX_OBSERVATION_MS
  ) {
    throw new Error("mobile smoke observation time is out of bounds");
  }
}

async function validatePackage(platform, packagePath) {
  const requested = path.resolve(packagePath);
  const metadata = await lstat(requested);
  if (metadata.isSymbolicLink()) {
    throw new Error("mobile smoke package must not be a symbolic link");
  }
  const expectedDirectory = platform === "ios";
  if (
    (expectedDirectory && !metadata.isDirectory()) ||
    (!expectedDirectory && !metadata.isFile())
  ) {
    throw new Error("mobile smoke package type is invalid");
  }
  const expectedExtension = platform === "ios" ? ".app" : ".apk";
  if (path.extname(requested).toLowerCase() !== expectedExtension) {
    throw new Error("mobile smoke package format is invalid");
  }
  return realpath(requested);
}

function delay(milliseconds) {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

async function defaultCommandRunner(file, args) {
  return execFileAsync(file, args, {
    encoding: "utf8",
    maxBuffer: MAX_CAPTURE_BYTES,
    timeout: COMMAND_TIMEOUT_MS,
  });
}

async function runCommand(commandRunner, stage, file, args, { allowFailure = false } = {}) {
  try {
    const result = await commandRunner(file, args);
    return {
      stdout: String(result?.stdout ?? ""),
      stderr: String(result?.stderr ?? ""),
    };
  } catch {
    if (allowFailure) {
      return { stdout: "", stderr: "" };
    }
    throw new Error(`mobile smoke command failed during ${stage}`);
  }
}

function exactAndroidDevice(device) {
  if (
    device.length > 128 ||
    !/^[A-Za-z0-9._:-]+$/u.test(device) ||
    device === "bootloader" ||
    device === "offline"
  ) {
    throw new Error("Android smoke device identifier is invalid");
  }
  return device;
}

function exactIosDevice(device) {
  if (!/^[0-9A-Fa-f-]{36}$/u.test(device)) {
    throw new Error("iOS smoke device identifier is invalid");
  }
  return device;
}

function parseAndroidPid(raw) {
  const value = raw.trim();
  if (!/^[1-9]\d{0,9}$/u.test(value)) {
    throw new Error("Android package did not remain alive");
  }
  return value;
}

function parseIosPid(raw) {
  const match = raw.trim().match(/^com\.filament\.desktop:\s+([1-9]\d{0,9})$/u);
  if (!match) {
    throw new Error("iOS package did not launch");
  }
  return Number.parseInt(match[1], 10);
}

function rejectMobileCrashLog(raw) {
  if (
    /FATAL EXCEPTION|Fatal signal|panicked at|hardened Tauri runtime failed|platform credential store is unavailable/iu.test(
      raw,
    )
  ) {
    throw new Error("mobile package reported a native startup failure");
  }
}

async function observeAndroid({
  device,
  observationMs,
  commandRunner,
  delayRunner,
}) {
  const adb = (stage, args, options) =>
    runCommand(commandRunner, stage, "adb", ["-s", device, ...args], options);

  await adb("log reset", ["logcat", "-c"]);
  await adb("package stop", ["shell", "am", "force-stop", APP_IDENTIFIER]);
  const resolved = await adb("activity resolution", [
    "shell",
    "cmd",
    "package",
    "resolve-activity",
    "--brief",
    APP_IDENTIFIER,
  ]);
  const component = resolved.stdout.trim();
  if (
    component.length > 256 ||
    !component.startsWith(`${APP_IDENTIFIER}/`) ||
    /[\r\n]/u.test(component)
  ) {
    throw new Error("Android package activity is invalid");
  }

  await adb("package launch", ["shell", "am", "start", "-W", "-n", component]);
  const initial = await adb("initial process inspection", [
    "shell",
    "pidof",
    "-s",
    APP_IDENTIFIER,
  ]);
  const initialPid = parseAndroidPid(initial.stdout);
  await delayRunner(observationMs);
  const observed = await adb("process observation", [
    "shell",
    "pidof",
    "-s",
    APP_IDENTIFIER,
  ]);
  if (parseAndroidPid(observed.stdout) !== initialPid) {
    throw new Error("Android package restarted during observation");
  }
  const logs = await adb("bounded crash-log inspection", [
    "logcat",
    "-d",
    "-v",
    "brief",
    "--pid",
    initialPid,
  ]);
  rejectMobileCrashLog(`${logs.stdout}\n${logs.stderr}`);
  return initialPid;
}

async function smokeAndroid({
  packagePath,
  device,
  observationMs,
  commandRunner,
  delayRunner,
}) {
  if (process.platform !== "linux" && commandRunner === defaultCommandRunner) {
    throw new Error("Android package smoke requires the Linux CI host");
  }
  const serial = exactAndroidDevice(device);
  const adb = (stage, args, options) =>
    runCommand(commandRunner, stage, "adb", ["-s", serial, ...args], options);

  const state = await adb("device readiness", ["get-state"]);
  if (state.stdout.trim() !== "device") {
    throw new Error("Android smoke device is unavailable");
  }
  await adb("offline policy", ["shell", "cmd", "connectivity", "airplane-mode", "enable"]);
  const airplaneMode = await adb("offline policy verification", [
    "shell",
    "settings",
    "get",
    "global",
    "airplane_mode_on",
  ]);
  if (airplaneMode.stdout.trim() !== "1") {
    throw new Error("Android smoke device did not enter airplane mode");
  }

  await adb("stale package removal", ["uninstall", APP_IDENTIFIER], { allowFailure: true });
  await adb("package installation", ["install", "--no-streaming", packagePath]);
  await observeAndroid({ device: serial, observationMs, commandRunner, delayRunner });
  await observeAndroid({ device: serial, observationMs, commandRunner, delayRunner });

  await adb("package uninstall", ["uninstall", APP_IDENTIFIER]);
  await adb("package reinstallation", ["install", "--no-streaming", packagePath]);
  await observeAndroid({ device: serial, observationMs, commandRunner, delayRunner });
  await adb("final package stop", ["shell", "am", "force-stop", APP_IDENTIFIER]);
}

async function observeIos({
  device,
  observationMs,
  commandRunner,
  delayRunner,
  processAlive,
}) {
  const launched = await runCommand(commandRunner, "package launch", "xcrun", [
    "simctl",
    "launch",
    "--terminate-running-process",
    device,
    APP_IDENTIFIER,
  ]);
  const pid = parseIosPid(launched.stdout);
  await delayRunner(observationMs);
  if (!processAlive(pid)) {
    throw new Error("iOS package exited during observation");
  }
  return pid;
}

async function smokeIos({
  packagePath,
  device,
  observationMs,
  commandRunner,
  delayRunner,
  processAlive,
}) {
  if (process.platform !== "darwin" && commandRunner === defaultCommandRunner) {
    throw new Error("iOS package smoke requires the macOS CI host");
  }
  const udid = exactIosDevice(device);
  const simctl = (stage, args, options) =>
    runCommand(commandRunner, stage, "xcrun", ["simctl", ...args], options);

  await simctl("simulator readiness", ["bootstatus", udid, "-b"]);
  await simctl("stale package removal", ["uninstall", udid, APP_IDENTIFIER], {
    allowFailure: true,
  });
  await simctl("package installation", ["install", udid, packagePath]);
  await observeIos({
    device: udid,
    observationMs,
    commandRunner,
    delayRunner,
    processAlive,
  });
  await observeIos({
    device: udid,
    observationMs,
    commandRunner,
    delayRunner,
    processAlive,
  });

  await simctl("package uninstall", ["uninstall", udid, APP_IDENTIFIER]);
  await simctl("package reinstallation", ["install", udid, packagePath]);
  const finalPid = await observeIos({
    device: udid,
    observationMs,
    commandRunner,
    delayRunner,
    processAlive,
  });
  await simctl("final package stop", ["terminate", udid, APP_IDENTIFIER]);
  await delayRunner(250);
  if (processAlive(finalPid)) {
    throw new Error("iOS package did not terminate after the smoke run");
  }
}

export async function runMobilePackageSmoke({
  platform,
  packagePath,
  device,
  evidencePath,
  observationMs = DEFAULT_OBSERVATION_MS,
  commandRunner = defaultCommandRunner,
  delayRunner = delay,
  processAlive = (pid) => {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  },
}) {
  if (platform !== "android" && platform !== "ios") {
    throw new Error("mobile smoke platform is unsupported");
  }
  validateObservationTime(observationMs);
  const canonicalPackage = await validatePackage(platform, packagePath);
  const canonicalEvidencePath = path.resolve(evidencePath);

  if (platform === "android") {
    await smokeAndroid({
      packagePath: canonicalPackage,
      device,
      observationMs,
      commandRunner,
      delayRunner,
    });
  } else {
    await smokeIos({
      packagePath: canonicalPackage,
      device,
      observationMs,
      commandRunner,
      delayRunner,
      processAlive,
    });
  }

  const evidence = {
    schema_version: 1,
    platform,
    package: path.basename(canonicalPackage),
    application_identifier: APP_IDENTIFIER,
    observation_ms: observationMs,
    installed_launch: true,
    restart_launch: true,
    uninstall_reinstall_launch: true,
    native_startup_fail_closed: true,
    offline_bundle_launch: true,
  };
  await mkdir(path.dirname(canonicalEvidencePath), { recursive: true });
  await writeFile(canonicalEvidencePath, `${JSON.stringify(evidence, null, 2)}\n`, {
    encoding: "utf8",
    mode: 0o600,
  });
  return evidence;
}

function isMainModule() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}

if (isMainModule()) {
  runMobilePackageSmoke(parseArguments(process.argv.slice(2))).catch((error) => {
    const message = error instanceof Error ? error.message : "mobile package smoke failed";
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  });
}
