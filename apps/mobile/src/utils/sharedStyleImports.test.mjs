import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const SRC_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function vueFiles(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const nested = await Promise.all(entries.map((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return vueFiles(full);
    return entry.name.endsWith(".vue") ? [full] : [];
  }));
  return nested.flat();
}

/*
 * Vite compiles a `<style src>` file once per production build. When several
 * components pull in the same file that way and one of them also has scoped
 * styles, the shared CSS can come out scoped to that one component and silently
 * stop applying everywhere else (the 2026-10-19 "white buttons" regression in
 * Settings). Shared stylesheets must be imported from the script instead.
 */
test("no stylesheet is pulled in by <style src> from more than one component", async () => {
  const usersByStylesheet = new Map();
  for (const file of await vueFiles(SRC_ROOT)) {
    const source = await readFile(file, "utf8");
    for (const match of source.matchAll(/<style\b[^>]*\bsrc="([^"]+)"[^>]*>/g)) {
      const stylesheet = path.resolve(path.dirname(file), match[1]);
      const users = usersByStylesheet.get(stylesheet) ?? [];
      users.push(path.relative(SRC_ROOT, file));
      usersByStylesheet.set(stylesheet, users);
    }
  }
  const shared = [...usersByStylesheet.entries()]
    .filter(([, users]) => users.length > 1)
    .map(([stylesheet, users]) => `${path.relative(SRC_ROOT, stylesheet)} <- ${users.join(", ")}`);
  assert.deepEqual(shared, [], "import shared stylesheets from <script> instead of <style src>");
});
