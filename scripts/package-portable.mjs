#!/usr/bin/env node

import { spawn } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const desktopRoot = join(repositoryRoot, "crates", "openconkit-desktop");
const windowsTarget = "x86_64-pc-windows-msvc";

function fail(message) {
  throw new Error(`package-portable: ${message}`);
}

function parseArguments(argv) {
  const options = {
    targetDirectory: join(repositoryRoot, "target"),
    outputDirectory: join(repositoryRoot, "output"),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--target-dir" && value) {
      options.targetDirectory = resolve(value);
      index += 1;
    } else if (argument === "--output-dir" && value) {
      options.outputDirectory = resolve(value);
      index += 1;
    } else {
      fail(`unsupported argument: ${argument ?? ""}`);
    }
  }
  return options;
}

async function requireRegularFile(path, label) {
  let metadata;
  try {
    metadata = await stat(path);
  } catch {
    fail(`${label} is missing: ${path}`);
  }
  if (!metadata.isFile()) {
    fail(`${label} is not a regular file: ${path}`);
  }
}

async function run(command, args) {
  return await new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      cwd: repositoryRoot,
      shell: false,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stderr = [];
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolvePromise();
      } else {
        reject(
          new Error(
            `${command} exited ${String(code)}: ${
              Buffer.concat(stderr).toString("utf8").trim() || "no diagnostic"
            }`,
          ),
        );
      }
    });
  });
}

async function copy(source, destination, label) {
  await requireRegularFile(source, label);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination);
}

async function main() {
  if (process.platform !== "win32") {
    fail("portable Windows packages must be assembled on Windows");
  }
  await run(process.execPath, [join(repositoryRoot, "scripts", "generate-notices.mjs"), "--check"]);
  const options = parseArguments(process.argv.slice(2));
  const version = (await readFile(join(repositoryRoot, "VERSION"), "utf8")).trim();
  if (
    !/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(
      version,
    )
  ) {
    fail("VERSION is not a semantic version");
  }

  const executable = join(
    options.targetDirectory,
    windowsTarget,
    "release",
    "openconkit-desktop.exe",
  );
  const sidecar = join(desktopRoot, "binaries", `codex-app-server-${windowsTarget}.exe`);
  const temporaryRoot = await mkdtemp(join(tmpdir(), "openconkit-portable-"));
  const packageName = `OpenConKit_${version}_windows_x64_portable`;
  const packageRoot = join(temporaryRoot, packageName);
  const archive = join(options.outputDirectory, `${packageName}.zip`);

  try {
    await mkdir(packageRoot, { recursive: true });
    await copy(executable, join(packageRoot, "OpenConKit.exe"), "OpenConKit executable");
    await copy(sidecar, join(packageRoot, "codex-app-server.exe"), "Codex app-server");
    await copy(
      join(repositoryRoot, "LICENSE"),
      join(packageRoot, "licenses", "OpenConKit-LICENSE.txt"),
      "OpenConKit license",
    );
    await copy(
      join(repositoryRoot, "NOTICE"),
      join(packageRoot, "licenses", "OpenConKit-NOTICE.txt"),
      "OpenConKit notice",
    );
    await copy(
      join(repositoryRoot, "THIRD_PARTY_NOTICES.md"),
      join(packageRoot, "licenses", "THIRD_PARTY_NOTICES.md"),
      "third-party notices",
    );
    for (const resource of [
      "LICENSE.txt",
      "NOTICE.txt",
      "codex_app_server_protocol.v2.schemas.json",
    ]) {
      await copy(
        join(desktopRoot, "resources", "codex", resource),
        join(packageRoot, "codex", resource),
        `Codex ${resource}`,
      );
    }
    await writeFile(join(packageRoot, "OPENCONKIT_PORTABLE"), "portable\n", {
      encoding: "utf8",
      flag: "wx",
    });
    await writeFile(
      join(packageRoot, "PORTABLE_README.txt"),
      [
        "OpenConKit portable for Windows",
        "",
        "Run OpenConKit.exe from this folder. Keep codex-app-server.exe beside it.",
        "OpenConKit stores application data under %USERPROFILE%\\.openconkit.",
        "This package does not edit source workbooks and does not contain telemetry.",
        "Portable builds cannot update themselves in place; Settings opens the official release page.",
        "Microsoft Edge WebView2 Runtime is required. Current Windows 10 and Windows 11 installations normally include it.",
        "",
        "OpenConKit المحمول لنظام Windows",
        "",
        "شغّل OpenConKit.exe من هذا المجلد، وأبقِ codex-app-server.exe بجواره.",
        "تُحفظ بيانات التطبيق داخل %USERPROFILE%\\.openconkit.",
        "لا تعدّل هذه الحزمة المصنفات المصدر ولا تحتوي على قياس عن بُعد.",
        "لا تحدّث النسخة المحمولة نفسها؛ تفتح صفحة الإعدادات صفحة الإصدار الرسمية.",
        "يلزم Microsoft Edge WebView2 Runtime، وهو مرفق عادةً مع الإصدارات الحالية من Windows 10 وWindows 11.",
        "",
      ].join("\r\n"),
      { encoding: "utf8", flag: "wx" },
    );

    await mkdir(options.outputDirectory, { recursive: true });
    await rm(archive, { force: true });
    await run("tar.exe", ["-a", "-c", "-f", archive, "-C", temporaryRoot, basename(packageRoot)]);
    await requireRegularFile(archive, "portable archive");
    const archiveSize = (await stat(archive)).size;
    if (archiveSize === 0) {
      fail("portable archive is empty");
    }
    process.stdout.write(`Created ${archive} (${archiveSize} bytes)\n`);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
