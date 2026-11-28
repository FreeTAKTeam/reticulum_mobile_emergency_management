// Parser-backed TypeScript/Vue import graph for archgraph (archgraph/graph/v1).
//
// A component is a source directory plus its first sub-directory (`apps/mobile/src/stores`);
// files directly in a source directory form the root component (`apps/mobile/src`).
// An edge `from -> to` means `from` imports `to`. Unresolvable imports go to `unresolved`.
//
//   node tools/architecture/ts-import-graph.mjs --root . --source apps/mobile/src \
//     --source packages/node-client/src --test-source e2e \
//     --alias @reticulum/node-client=packages/node-client/src/index.ts --out graph.json
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";

import { parse as parseSfc } from "@vue/compiler-sfc";
import ts from "typescript";

const EXTS = [".ts", ".tsx", ".vue", ".js", ".mjs"];
const SKIP_DIRS = new Set(["node_modules", "dist", "build", ".git"]);
const TEST_FILE = /\.(test|spec)\.[cm]?[jt]sx?$/;

const allTypeOnly = (elements) => elements.length > 0 && elements.every((e) => e.isTypeOnly);

function isTypeOnlyImport(node) {
  const clause = node.importClause;
  if (!clause) return false; // side-effect import runs code
  if (clause.isTypeOnly) return true;
  const bindings = clause.namedBindings;
  return !clause.name && bindings !== undefined && ts.isNamedImports(bindings) && allTypeOnly(bindings.elements);
}

function isTypeOnlyExport(node) {
  if (node.isTypeOnly) return true;
  const clause = node.exportClause;
  return clause !== undefined && ts.isNamedExports(clause) && allTypeOnly(clause.elements);
}

/** Imports of one TS/JS source text: [{ spec: string | null, kind, line, detail }]. */
export function extractImports(text, fileName = "file.ts", lineOffset = 0) {
  const kind = /\.[cm]?jsx?$/.test(fileName) ? ts.ScriptKind.JS : ts.ScriptKind.TS;
  const sf = ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, true, kind);
  const found = [];
  const add = (node, spec, edgeKind) => {
    const line = sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1 + lineOffset;
    found.push({ spec, kind: edgeKind, line, detail: node.getText(sf).replace(/\s+/g, " ").slice(0, 160) });
  };
  const literal = (node) =>
    node && (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) ? node.text : null;

  const visit = (node) => {
    if (ts.isImportDeclaration(node)) {
      add(node, literal(node.moduleSpecifier), isTypeOnlyImport(node) ? "type-import" : "import");
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier) {
      add(node, literal(node.moduleSpecifier), isTypeOnlyExport(node) ? "type-import" : "import");
    } else if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
      add(node, literal(node.moduleReference.expression), node.isTypeOnly ? "type-import" : "import");
    } else if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
      add(node, literal(node.arguments[0]), "dynamic-import");
    } else if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)) {
      add(node, literal(node.argument.literal), "type-import");
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  return found;
}

/** Imports of a Vue SFC: both `<script>` blocks, with file line numbers. */
export function extractVueImports(text, fileName = "file.vue") {
  const { descriptor, errors } = parseSfc(text, { filename: fileName, sourceMap: false });
  if (errors.length > 0) throw new Error(`${fileName}: ${errors[0].message ?? errors[0]}`);
  return [descriptor.script, descriptor.scriptSetup]
    .filter(Boolean)
    .flatMap((block) => {
      const lang = block.lang === "js" || !block.lang ? "x.js" : `x.${block.lang}`;
      return extractImports(block.content, lang, block.loc.start.line - 1);
    });
}

function resolveFile(base, spec) {
  const target = path.resolve(base, spec);
  const candidates = [target, ...EXTS.map((e) => target + e)];
  if (target.endsWith(".js")) candidates.push(target.slice(0, -3) + ".ts");
  candidates.push(...EXTS.map((e) => path.join(target, "index" + e)));
  return candidates.find((c) => existsSync(c) && statSync(c).isFile()) ?? null;
}

function listFiles(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .sort((a, b) => a.name.localeCompare(b.name))
    .flatMap((entry) => {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) return SKIP_DIRS.has(entry.name) ? [] : listFiles(full);
      return EXTS.some((e) => entry.name.endsWith(e)) && !entry.name.endsWith(".d.ts") ? [full] : [];
    });
}

function gitSource(root) {
  try {
    const run = (...args) => execFileSync("git", ["-C", root, ...args], { encoding: "utf8" }).trim();
    return { commit: run("rev-parse", "HEAD"), dirty: run("status", "--porcelain", "--untracked-files=no") !== "" };
  } catch {
    return {};
  }
}

export function buildGraph({ root, sources, testSources = [], aliases = {}, schema = "archgraph/graph/v1" }) {
  root = path.resolve(root);
  const allSources = [
    ...sources.map((s) => [s.replace(/\/+$/, ""), false]),
    ...testSources.map((s) => [s.replace(/\/+$/, ""), true]),
  ];
  const componentOf = (file) => {
    const rel = path.relative(root, file).split(path.sep).join("/");
    for (const [src, isTest] of allSources) {
      if (rel.startsWith(src + "/")) {
        const rest = rel.slice(src.length + 1).split("/");
        return { id: rest.length === 1 ? src : `${src}/${rest[0]}`, isTest };
      }
    }
    return null;
  };

  const files = allSources.flatMap(([src]) => listFiles(path.join(root, src)));
  const components = new Map();
  for (const f of files) {
    const id = componentOf(f).id;
    if (!components.has(id)) components.set(id, path.relative(root, f).split(path.sep).join("/"));
  }

  const edges = new Map();
  const unresolved = [];
  const thirdParty = new Map();
  let intra = 0;
  let assets = 0;
  for (const f of files) {
    const { id: from, isTest: inTestSource } = componentOf(f);
    const rel = path.relative(root, f).split(path.sep).join("/");
    const isTest = inTestSource || TEST_FILE.test(f);
    const text = readFileSync(f, "utf8");
    const found = f.endsWith(".vue") ? extractVueImports(text, rel) : extractImports(text, rel);
    for (const { spec, kind: baseKind, line, detail } of found) {
      const kind = isTest ? `test-${baseKind}` : baseKind;
      const evidence = { location: rel, line, detail };
      const miss = (target, reason) => unresolved.push({ from, target, kind, reason, evidence: [evidence] });
      if (spec === null) {
        miss(detail, "computed module specifier");
        continue;
      }
      const alias = Object.keys(aliases).find((a) => spec === a || spec.startsWith(a + "/"));
      let target;
      if (alias) {
        target = resolveFile(root, aliases[alias] + spec.slice(alias.length));
      } else if (spec.startsWith("/")) {
        miss(spec, "root-absolute specifier; bundler root not known");
        continue;
      } else if (spec.startsWith("@/") || spec.startsWith("~")) {
        miss(spec, "unconfigured alias; pass --alias");
        continue;
      } else if (spec.startsWith(".")) {
        target = resolveFile(path.dirname(f), spec);
      } else {
        const pkg = spec.split("/").slice(0, spec.startsWith("@") ? 2 : 1).join("/");
        thirdParty.set(pkg, (thirdParty.get(pkg) ?? 0) + 1);
        continue;
      }
      if (target && !EXTS.some((e) => target.endsWith(e))) {
        assets += 1;
        continue;
      }
      const to = target ? componentOf(target)?.id : undefined;
      if (!to) {
        miss(spec, target ? "target outside analysed sources" : "file not found");
        continue;
      }
      if (to === from) {
        intra += 1;
        continue;
      }
      const key = JSON.stringify([from, to, kind]);
      if (!edges.has(key)) edges.set(key, []);
      edges.get(key).push(evidence);
    }
  }

  const statements = [...thirdParty.values()].reduce((a, b) => a + b, 0);
  return {
    schema,
    source: {
      extractor: "ts-import-graph (typescript AST + @vue/compiler-sfc)",
      ...gitSource(root),
      settings: {
        sources: sources.join(","),
        test_sources: testSources.join(","),
        aliases: Object.entries(aliases).sort().map(([k, v]) => `${k}=${v}`).join(","),
      },
      limitations: [
        "parser-backed: imports inside comments and strings are ignored; require() and CSS @import are not read",
        "type-import = `import type`, `export type`, all-`type` named bindings, or `import(\"x\").T` type nodes",
        "*.test.*/*.spec.* files and --test-source directories give test-* edge kinds",
        `${intra} intra-component imports not shown in the DSM; ${assets} asset imports (css, images) not modelled`,
        `${statements} third-party import statements to ${thirdParty.size} packages not modelled: ${[...thirdParty.keys()].sort().join(", ")}`,
      ],
    },
    components: [...components.entries()].sort(([a], [b]) => a.localeCompare(b))
      .map(([id, location]) => ({ id, kind: "module", location })),
    edges: [...edges.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([key, evidence]) => {
      const [from, to, kind] = JSON.parse(key);
      return { from, to, kind, evidence };
    }),
    unresolved,
  };
}

function main() {
  const { values } = parseArgs({
    options: {
      root: { type: "string", default: "." },
      source: { type: "string", multiple: true },
      "test-source": { type: "string", multiple: true, default: [] },
      alias: { type: "string", multiple: true, default: [] },
      schema: { type: "string", default: "archgraph/graph/v1" },
      out: { type: "string" },
    },
  });
  if (!values.source?.length) throw new Error("at least one --source is required");
  const aliases = Object.fromEntries(values.alias.map((a) => {
    const at = a.indexOf("=");
    return [a.slice(0, at), a.slice(at + 1)];
  }));
  const graph = buildGraph({
    root: values.root, sources: values.source, testSources: values["test-source"], aliases, schema: values.schema,
  });
  const text = JSON.stringify(graph, null, 2) + "\n";
  if (values.out) writeFileSync(values.out, text);
  else process.stdout.write(text);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
