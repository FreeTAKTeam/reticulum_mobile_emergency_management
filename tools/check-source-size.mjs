import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const MAX_LINES = 500;
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ALLOWLIST_PATH = path.join(ROOT, "tools", "source-size-allowlist.json");
const SOURCE_EXTENSIONS = new Set([
  ".java",
  ".js",
  ".kt",
  ".mjs",
  ".ps1",
  ".rs",
  ".sh",
  ".ts",
  ".tsx",
  ".vue",
]);
const EXCLUDED_SEGMENTS = new Set([
  "build",
  "generated",
  "jniLibs",
  "node_modules",
  "target",
  "uniffi",
  "vendor",
]);
const EXCLUDED_PATHS = new Set(["stitch_app.js", "stitch_base.js"]);
const EXCLUDED_PREFIXES = [
  "apps/mobile/android/app/src/main/java/com/nonpolynomial/",
];

function normalizePath(filePath) {
  return filePath.replaceAll("\\", "/");
}

function isFirstPartySource(filePath) {
  if (EXCLUDED_PATHS.has(filePath)) return false;
  if (EXCLUDED_PREFIXES.some((prefix) => filePath.startsWith(prefix))) return false;
  if (!SOURCE_EXTENSIONS.has(path.posix.extname(filePath))) return false;
  return !filePath.split("/").some((segment) => EXCLUDED_SEGMENTS.has(segment));
}

function physicalLineCount(contents) {
  if (contents.length === 0) return 0;
  return contents.split(/\r?\n/u).length - (contents.endsWith("\n") ? 1 : 0);
}

function repositoryFiles() {
  const output = execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    { cwd: ROOT, encoding: "utf8" },
  );
  return output
    .split("\0")
    .filter(Boolean)
    .map(normalizePath)
    .filter(isFirstPartySource)
    .sort();
}

const allowlist = JSON.parse(readFileSync(ALLOWLIST_PATH, "utf8"));
const files = repositoryFiles();
const fileSet = new Set(files);
const failures = [];
let oversizedCount = 0;

for (const filePath of files) {
  const lineCount = physicalLineCount(readFileSync(path.join(ROOT, filePath), "utf8"));
  if (lineCount <= MAX_LINES) continue;

  oversizedCount += 1;
  const allowedMaximum = allowlist[filePath];
  if (allowedMaximum === undefined) {
    failures.push(`${filePath}: ${lineCount} lines (new violation; maximum is ${MAX_LINES})`);
  } else if (lineCount > allowedMaximum) {
    failures.push(`${filePath}: grew from allowed ${allowedMaximum} to ${lineCount} lines`);
  }
}

for (const [filePath, allowedMaximum] of Object.entries(allowlist)) {
  if (!fileSet.has(filePath)) {
    failures.push(`${filePath}: stale allowlist entry; file no longer exists`);
    continue;
  }

  const lineCount = physicalLineCount(readFileSync(path.join(ROOT, filePath), "utf8"));
  if (lineCount <= MAX_LINES) {
    failures.push(`${filePath}: now ${lineCount} lines; remove its stale allowlist entry`);
  } else if (allowedMaximum <= MAX_LINES) {
    failures.push(`${filePath}: allowlist maximum ${allowedMaximum} must exceed ${MAX_LINES}`);
  }
}

if (failures.length > 0) {
  console.error(`Source-size gate failed (${failures.length} issue${failures.length === 1 ? "" : "s"}):`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(
    `Source-size gate passed: ${files.length} first-party files checked, ${oversizedCount} ratcheted exception${oversizedCount === 1 ? "" : "s"}.`,
  );
}
