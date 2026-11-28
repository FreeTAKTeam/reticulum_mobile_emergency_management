import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { buildGraph, extractImports, extractVueImports } from "./ts-import-graph.mjs";

const specs = (found) => found.map(({ spec, kind, line }) => [spec, kind, line]);

test("ignores imports inside comments and strings", () => {
  const source = [
    "// import { a } from \"./commented\";",
    "/* import b from \"./block\"; */",
    "const s = \"import c from './string'\";",
    "const t = `import(\"./template\")`;",
    "import { real } from \"./real\";",
  ].join("\n");
  assert.deepEqual(specs(extractImports(source)), [["./real", "import", 5]]);
});

test("classifies type-only and runtime imports", () => {
  const source = [
    "import type { A } from \"./a\";",
    "import { type B, type C } from \"./b\";",
    "import { type D, e } from \"./d\";",
    "export type { F } from \"./f\";",
    "export { g } from \"./g\";",
    "import \"./side-effect\";",
    "let x: import(\"./h\").H;",
    "const m = await import(\"./lazy\");",
    "const n = await import(name);",
  ].join("\n");
  assert.deepEqual(specs(extractImports(source)), [
    ["./a", "type-import", 1],
    ["./b", "type-import", 2],
    ["./d", "import", 3],
    ["./f", "type-import", 4],
    ["./g", "import", 5],
    ["./side-effect", "import", 6],
    ["./h", "type-import", 7],
    ["./lazy", "dynamic-import", 8],
    [null, "dynamic-import", 9],
  ]);
});

test("reads both Vue script blocks with file line numbers", () => {
  const sfc = [
    "<template><div>import x from \"./template-text\"</div></template>",
    "<script lang=\"ts\">",
    "export type { P } from \"./props\";",
    "</script>",
    "<script setup lang=\"ts\">",
    "import Child from \"./Child.vue\";",
    "</script>",
  ].join("\n");
  assert.deepEqual(specs(extractVueImports(sfc)), [
    ["./props", "type-import", 3],
    ["./Child.vue", "import", 6],
  ]);
});

test("builds components, aliases, unresolved and test edges", () => {
  const root = mkdtempSync(path.join(tmpdir(), "ts-import-graph-"));
  const write = (rel, text) => {
    mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
    writeFileSync(path.join(root, rel), text);
  };
  write("pkg/src/index.ts", "export const api = 1;\n");
  write("app/src/main.ts", "import { api } from \"@pkg/client\";\nimport { u } from \"./utils/u\";\n");
  write("app/src/utils/u.ts", "import type { T } from \"../types/t\";\nimport \"./style.css\";\nexport const u = 1;\n");
  write("app/src/utils/style.css", "");
  write("app/src/types/t.ts", "export type T = 1;\n");
  write("app/src/views/V.vue", "<script setup lang=\"ts\">\nimport { u } from \"../utils/u\";\nimport x from \"./missing\";\nimport y from \"@/x\";\n</script>\n");
  write("app/src/utils/u.test.ts", "import { api } from \"@pkg/client\";\n");

  const graph = buildGraph({
    root,
    sources: ["app/src", "pkg/src"],
    aliases: { "@pkg/client": "pkg/src/index.ts" },
  });
  assert.deepEqual(graph.components.map((c) => c.id), [
    "app/src", "app/src/types", "app/src/utils", "app/src/views", "pkg/src",
  ]);
  assert.deepEqual(graph.edges.map((e) => [e.from, e.to, e.kind]), [
    ["app/src", "app/src/utils", "import"],
    ["app/src", "pkg/src", "import"],
    ["app/src/utils", "app/src/types", "type-import"],
    ["app/src/utils", "pkg/src", "test-import"],
    ["app/src/views", "app/src/utils", "import"],
  ]);
  assert.deepEqual(graph.unresolved.map((u) => [u.target, u.reason]), [
    ["./missing", "file not found"],
    ["@/x", "unconfigured alias; pass --alias"],
  ]);
  assert.equal(graph.edges.find((e) => e.from === "app/src/views").evidence[0].line, 2);

  const files = buildGraph({ root, sources: ["app/src", "pkg/src"], aliases: { "@pkg/client": "pkg/src/index.ts" }, granularity: "file" });
  assert.ok(files.edges.some((e) => e.from === "app/src/views/V.vue" && e.to === "app/src/utils/u.ts"));
});
