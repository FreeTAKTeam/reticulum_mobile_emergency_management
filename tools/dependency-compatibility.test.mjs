import { createRequire } from "node:module";
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, access, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const require = createRequire(new URL("../package.json", import.meta.url));
const { rimraf, rimrafSync } = require("rimraf");
const { glob, globSync } = require("glob");

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "rem-dependency-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const names = [
    "a.tmp",
    "b.tmp",
    "keep.txt",
    "literal [x].txt",
    "nested/c.tmp",
    "nested/keep.txt",
  ];
  for (const name of names) {
    await mkdir(dirname(join(root, name)), { recursive: true });
    await writeFile(join(root, name), "fixture");
  }
  return root;
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

test("literal async/sync deletion preserves similarly named files", async (t) => {
  const root = await fixture(t);
  const target = join(root, "literal [x].txt");
  assert.equal(await rimraf(target), true);
  assert.equal(await exists(target), false);
  assert.equal(await exists(join(root, "keep.txt")), true);
  assert.equal(rimrafSync(join(root, "nested")), true);
  assert.equal(await exists(join(root, "nested")), false);
});

test("async glob removal respects explicit ignore and absolute root", async (t) => {
  const root = await fixture(t);
  const removed = await rimraf(join(root, "**/*.tmp"), {
    glob: { ignore: join(root, "nested/**") },
  });
  assert.equal(removed, true);
  assert.equal(await exists(join(root, "a.tmp")), false);
  assert.equal(await exists(join(root, "nested/c.tmp")), true);
  assert.equal(await exists(join(root, "keep.txt")), true);
});

test("sync glob removal respects cwd and no-match behavior", async (t) => {
  const root = await fixture(t);
  assert.equal(rimrafSync("*.tmp", { glob: { cwd: root } }), true);
  assert.equal(await exists(join(root, "b.tmp")), false);
  assert.equal(await exists(join(root, "nested/c.tmp")), true);
  assert.equal(rimrafSync("nothing-*.missing", { glob: { cwd: root } }), true);
});

test("public glob/globSync APIs return equivalent string paths", async (t) => {
  const root = await fixture(t);
  const options = { cwd: root, absolute: true, withFileTypes: false };
  const asyncResult = await glob("**/*.tmp", options);
  assert.deepEqual(asyncResult.sort(), globSync("**/*.tmp", options).sort());
  assert.equal(asyncResult.length, 3);
  assert.ok(asyncResult.every((path) => typeof path === "string" && path.startsWith(root)));
});

test("pre-aborted traversal rejects and rimraf leaves fixture intact", async (t) => {
  const root = await fixture(t);
  const controller = new AbortController();
  controller.abort(new Error("cancelled-fixture"));
  await assert.rejects(
    glob("**/*", { cwd: root, signal: controller.signal }),
    /cancelled-fixture/,
  );
  await assert.rejects(
    rimraf(join(root, "**/*"), { glob: { signal: controller.signal } }),
    /cancelled-fixture/,
  );
  assert.equal(await exists(join(root, "a.tmp")), true);
});
