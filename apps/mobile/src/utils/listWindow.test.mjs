import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./listWindow.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { listWindowBounds, sliceListWindow } = await import(moduleUrl);

test("large collections render in bounded 200-row windows", () => {
  const records = Array.from({ length: 1_000 }, (_, index) => index);
  const pages = Array.from({ length: 5 }, (_, page) => sliceListWindow(records, page));

  assert.deepEqual(pages.map((items) => items.length), [200, 200, 200, 200, 200]);
  assert.equal(pages[0][0], 0);
  assert.equal(pages[4][199], 999);
});

test("window bounds clamp invalid pages without dropping the final partial page", () => {
  assert.deepEqual(listWindowBounds(451, 99), {
    page: 2,
    pageCount: 3,
    startIndex: 400,
    endIndex: 451,
  });
  assert.deepEqual(listWindowBounds(0, -1), {
    page: 0,
    pageCount: 1,
    startIndex: 0,
    endIndex: 0,
  });
});
