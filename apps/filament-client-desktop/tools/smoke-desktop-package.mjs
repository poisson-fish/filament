#!/usr/bin/env node

import { execFile } from "node:child_process";
import { constants as fsConstants } from "node:fs";
import { access, lstat, mkdir, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const MAX_CAPTURE_BYTES = 64 * 1024;
const MIN_OBSERVATION_MS = 250;
const MAX_OBSERVATION_MS = 30_000;
const SOCKET_SAMPLE_INTERVAL_MS = 500;
const TERMINATION_GRACE_MS = 500;
const SUPPORTED_PLATFORMS = new Map([
  ["linux", "linux"],
  ["macos", "darwin"],
  ["windows", "win32"],
]);

const ENVIRONMENT_ALLOWLIST = [
  "APPDATA",
  "APPIMAGE_EXTRACT_AND_RUN",
  "ComSpec",
  "DBUS_SESSION_BUS_ADDRESS",
  "DISPLAY",
  "HOME",
  "LANG",
  "LC_ALL",
  "LOCALAPPDATA",
  "LOGNAME",
  "PATH",
  "PATHEXT",
  "PSModulePath",
  "Path",
  "ProgramData",
  "ProgramFiles",
  "ProgramFiles(x86)",
  "SystemRoot",
  "TEMP",
  "TMP",
  "TMPDIR",
  "USER",
  "USERPROFILE",
  "WAYLAND_DISPLAY",
  "WINDIR",
  "XAUTHORITY",
  "XDG_RUNTIME_DIR",
];

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

  const allowed = new Set(["--platform", "--executable", "--evidence", "--observation-ms"]);
  for (const flag of Object.keys(options)) {
    if (!allowed.has(flag)) {
      throw new Error(`unknown argument: ${flag}`);
    }
  }
  for (const required of ["--platform", "--executable", "--evidence"]) {
    if (!options[required]) {
      throw new Error(`missing required argument: ${required}`);
    }
  }

  const observationValue = options["--observation-ms"];
  if (observationValue !== undefined && !/^\d+$/u.test(observationValue)) {
    throw new Error("observation time must be an integer");
  }
  const observationMs =
    observationValue === undefined ? 8_000 : Number.parseInt(observationValue, 10);
  if (
    !Number.isSafeInteger(observationMs) ||
    observationMs < MIN_OBSERVATION_MS ||
    observationMs > MAX_OBSERVATION_MS
  ) {
    throw new Error(
      `observation time must be ${MIN_OBSERVATION_MS}..=${MAX_OBSERVATION_MS} milliseconds`,
    );
  }

  return {
    platform: options["--platform"],
    executable: path.resolve(options["--executable"]),
    evidencePath: path.resolve(options["--evidence"]),
    observationMs,
  };
}

function smokeEnvironment(source) {
  const environment = {};
  for (const name of ENVIRONMENT_ALLOWLIST) {
    if (source[name] !== undefined) {
      environment[name] = source[name];
    }
  }
  return {
    ...environment,
    HTTP_PROXY: "http://127.0.0.1:9",
    HTTPS_PROXY: "http://127.0.0.1:9",
    ALL_PROXY: "http://127.0.0.1:9",
    NO_PROXY: "",
    http_proxy: "http://127.0.0.1:9",
    https_proxy: "http://127.0.0.1:9",
    all_proxy: "http://127.0.0.1:9",
    no_proxy: "",
  };
}

async function validateExecutable(executable) {
  const metadata = await lstat(executable);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    throw new Error("desktop smoke executable must be a regular non-symlink file");
  }
  await access(executable, fsConstants.X_OK);
  const canonical = await realpath(executable);
  const canonicalMetadata = await lstat(canonical);
  if (!canonicalMetadata.isFile()) {
    throw new Error("desktop smoke executable must resolve to a regular file");
  }
  return canonical;
}

function appendBounded(state, chunk) {
  state.bytes += chunk.length;
  if (state.bytes > MAX_CAPTURE_BYTES) {
    state.exceeded = true;
  }
}

function delay(milliseconds) {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

async function unixSocketCount(processGroupId) {
  try {
    const { stdout } = await execFileAsync(
      "lsof",
      ["-nP", "-a", "-g", String(processGroupId), "-i"],
      {
        encoding: "utf8",
        maxBuffer: MAX_CAPTURE_BYTES,
        timeout: 5_000,
      },
    );
    const lines = stdout
      .split(/\r?\n/u)
      .map((line) => line.trim())
      .filter(Boolean);
    return Math.max(0, lines.length - (lines[0]?.startsWith("COMMAND") ? 1 : 0));
  } catch (error) {
    if (error && typeof error === "object" && error.code === 1 && !error.stdout) {
      return 0;
    }
    throw new Error("desktop smoke network inspection failed");
  }
}

const WINDOWS_SOCKET_QUERY = String.raw`
$ErrorActionPreference = "Stop"
$rootPid = [int]$env:FILAMENT_SMOKE_ROOT_PID
$processes = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
$ids = [System.Collections.Generic.HashSet[int]]::new()
[void]$ids.Add($rootPid)
do {
  $changed = $false
  foreach ($process in $processes) {
    if ($ids.Contains([int]$process.ParentProcessId) -and $ids.Add([int]$process.ProcessId)) {
      $changed = $true
    }
  }
} while ($changed)
$count = @(
  Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue |
    Where-Object { $ids.Contains([int]$_.OwningProcess) }
).Count
Write-Output $count
`;

async function windowsSocketCount(rootPid) {
  try {
    const { stdout } = await execFileAsync(
      "powershell.exe",
      ["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", WINDOWS_SOCKET_QUERY],
      {
        encoding: "utf8",
        env: {
          ...smokeEnvironment(process.env),
          FILAMENT_SMOKE_ROOT_PID: String(rootPid),
        },
        maxBuffer: MAX_CAPTURE_BYTES,
        timeout: 10_000,
        windowsHide: true,
      },
    );
    const count = Number.parseInt(stdout.trim(), 10);
    if (!Number.isSafeInteger(count) || count < 0) {
      throw new Error("invalid socket count");
    }
    return count;
  } catch {
    throw new Error("desktop smoke network inspection failed");
  }
}

async function socketCount(platform, rootPid) {
  return platform === "windows"
    ? windowsSocketCount(rootPid)
    : unixSocketCount(rootPid);
}

async function terminate(child, platform) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  if (platform === "windows") {
    try {
      await execFileAsync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], {
        maxBuffer: MAX_CAPTURE_BYTES,
        timeout: 10_000,
        windowsHide: true,
      });
    } catch {
      // The process may have exited between inspection and teardown.
    }
    return;
  }

  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    return;
  }
  await delay(TERMINATION_GRACE_MS);
  try {
    process.kill(-child.pid, "SIGKILL");
  } catch {
    // The process group exited during the grace period.
  }
}

export async function runDesktopPackageSmoke({
  platform,
  executable,
  evidencePath,
  observationMs = 8_000,
  executableArgs = [],
}) {
  const expectedHost = SUPPORTED_PLATFORMS.get(platform);
  if (!expectedHost || expectedHost !== process.platform) {
    throw new Error("desktop smoke platform does not match the current host");
  }
  if (
    !Number.isSafeInteger(observationMs) ||
    observationMs < MIN_OBSERVATION_MS ||
    observationMs > MAX_OBSERVATION_MS
  ) {
    throw new Error("desktop smoke observation time is out of bounds");
  }

  const requestedExecutable = path.resolve(executable);
  const canonicalEvidencePath = path.resolve(evidencePath);
  const canonicalExecutable = await validateExecutable(requestedExecutable);

  const stdout = { bytes: 0, exceeded: false };
  const stderr = { bytes: 0, exceeded: false };
  const child = spawn(canonicalExecutable, executableArgs, {
    detached: platform !== "windows",
    env: smokeEnvironment(process.env),
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout.on("data", (chunk) => appendBounded(stdout, chunk));
  child.stderr.on("data", (chunk) => appendBounded(stderr, chunk));

  let observedSocketCount = 0;
  let earlyExit = null;
  const exitPromise = new Promise((resolve) => {
    child.once("error", (error) => {
      earlyExit = { kind: "spawn_error", error };
      resolve();
    });
    child.once("exit", (code, signal) => {
      earlyExit = { kind: "exit", code, signal };
      resolve();
    });
  });

  try {
    const deadline = Date.now() + observationMs;
    while (Date.now() < deadline && earlyExit === null) {
      const remaining = deadline - Date.now();
      await Promise.race([exitPromise, delay(Math.min(SOCKET_SAMPLE_INTERVAL_MS, remaining))]);
      if (earlyExit === null) {
        observedSocketCount += await socketCount(platform, child.pid);
      }
      if (stdout.exceeded || stderr.exceeded) {
        throw new Error("desktop smoke process output exceeded the bounded capture limit");
      }
      if (observedSocketCount > 0) {
        throw new Error("desktop package opened a network socket during offline launch");
      }
    }

    if (earlyExit !== null) {
      throw new Error("desktop package exited before the offline launch observation completed");
    }

    const evidence = {
      schema_version: 1,
      platform,
      executable: path.basename(canonicalExecutable),
      observation_ms: observationMs,
      process_alive: true,
      network_socket_count: observedSocketCount,
      stdout_bytes: stdout.bytes,
      stderr_bytes: stderr.bytes,
      offline_bundle_launch: true,
    };
    await mkdir(path.dirname(canonicalEvidencePath), { recursive: true });
    await writeFile(canonicalEvidencePath, `${JSON.stringify(evidence, null, 2)}\n`, {
      encoding: "utf8",
      mode: 0o600,
    });
    return evidence;
  } finally {
    await terminate(child, platform);
  }
}

function isMainModule() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}

if (isMainModule()) {
  runDesktopPackageSmoke(parseArguments(process.argv.slice(2))).catch((error) => {
    const message = error instanceof Error ? error.message : "desktop package smoke failed";
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  });
}
