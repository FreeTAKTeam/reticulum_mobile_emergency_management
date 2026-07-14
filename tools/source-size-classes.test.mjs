import assert from "node:assert/strict";
import test from "node:test";

import {
  evaluateClassSizes,
  findClassSpans,
  supportsClassDeclarations,
} from "./source-size-classes.mjs";

test("counts named, nested, and anonymous class spans", () => {
  const source = [
    "class Outer {",
    "  class Inner {",
    "  }",
    "}",
    "export default class {",
    "}",
  ].join("\n");

  assert.deepEqual(findClassSpans(source), [
    { name: "Outer", startLine: 1, endLine: 4, lineCount: 4 },
    { name: "Inner", startLine: 2, endLine: 3, lineCount: 2 },
    { name: "<anonymous>", startLine: 5, endLine: 6, lineCount: 2 },
  ]);
});

test("ignores class text and braces inside comments and strings", () => {
  const source = [
    "// class Commented { }",
    "const example = `class Template { }`;",
    "class Real {",
    "  value = \"} class Fake {\";",
    "  /* class Blocked { } */",
    "}",
  ].join("\n");

  assert.deepEqual(findClassSpans(source), [
    { name: "Real", startLine: 3, endLine: 6, lineCount: 4 },
  ]);
});

test("ignores Vue attributes, CSS selectors, and Kotlin class references", () => {
  const source = [
    '<main class="layout">',
    "</main>",
    ".class { color: red; }",
    "val type = Example::class",
    "class Real {}",
  ].join("\n");

  assert.deepEqual(findClassSpans(source), [
    { name: "Real", startLine: 5, endLine: 5, lineCount: 1 },
  ]);
});

test("ignores class text and unmatched braces inside regex literals", () => {
  const source = [
    "class RegexOwner {",
    "  closing = /}/;",
    "  opening = /{/;",
    "  named = /class Fake \\{}/;",
    "}",
  ].join("\n");

  assert.deepEqual(findClassSpans(source), [
    { name: "RegexOwner", startLine: 1, endLine: 5, lineCount: 5 },
  ]);
});

test("keeps duplicate class names stable without line-number keys", () => {
  const source = "class Reused {}\nclass Reused {}";

  assert.deepEqual(
    findClassSpans(source).map(({ name }) => name),
    ["Reused", "Reused[2]"],
  );
});

test("limits class scanning to supported source formats", () => {
  assert.equal(supportsClassDeclarations("src/client.ts"), true);
  assert.equal(supportsClassDeclarations("src/Plugin.java"), true);
  assert.equal(supportsClassDeclarations("src/runtime.rs"), false);
});

test("rejects new, growing, and stale class exceptions", () => {
  const classesByFile = new Map([
    [
      "src/Client.ts",
      new Map([
        ["Growing", { name: "Growing", lineCount: 620 }],
        ["NewClass", { name: "NewClass", lineCount: 510 }],
        ["NowSmall", { name: "NowSmall", lineCount: 400 }],
      ]),
    ],
  ]);
  const allowlist = {
    "src/Client.ts": {
      Growing: 600,
      Missing: 700,
      NowSmall: 700,
    },
    "src/Removed.ts": { Removed: 800 },
  };

  assert.deepEqual(evaluateClassSizes(classesByFile, allowlist, 500), {
    oversizedCount: 2,
    failures: [
      "src/Client.ts#Growing: grew from allowed 600 to 620 lines",
      "src/Client.ts#NewClass: 510 lines (new class violation; maximum is 500)",
      "src/Client.ts#Missing: stale class allowlist entry; class no longer exists",
      "src/Client.ts#NowSmall: now 400 lines; remove its stale allowlist entry",
      "src/Removed.ts: stale class allowlist entry; file does not support class declarations",
    ],
  });
});
