import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./mapMarkerPresentation.ts", import.meta.url), "utf8");
const output = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(output).toString("base64")}`;
const {
  autoMarkerName,
  categoryLabel,
  groupSymbols,
  markerAgeLabel,
  pushRecentSymbol,
  recentSymbols,
  searchSymbols,
} = await import(moduleUrl);

const symbol = (id, description, category, aliases = []) => ({
  id, set: "mdi", mdi: id, description, category, aliases,
});
const catalog = [
  symbol("marker", "Marker", "general", ["pin"]),
  symbol("friendly", "Friendly", "nato"),
  symbol("hostile", "Hostile", "nato"),
  symbol("vehicle", "Vehicle", "mobility", ["car", "truck"]),
  symbol("medic", "Medic", "medical", ["hospital"]),
];

test("groups symbols by catalog category in catalog order", () => {
  const groups = groupSymbols(catalog);
  assert.deepEqual(groups.map((group) => group.label), ["General", "NATO", "Mobility", "Medical"]);
  assert.deepEqual(groups[1].symbols.map((entry) => entry.id), ["friendly", "hostile"]);
  assert.equal(categoryLabel("home-flood"), "Home Flood");
});

test("search matches labels, categories and RCH aliases", () => {
  assert.deepEqual(searchSymbols(catalog, "car").map((entry) => entry.id), ["vehicle"]);
  assert.deepEqual(searchSymbols(catalog, " HOSP ").map((entry) => entry.id), ["medic"]);
  assert.deepEqual(searchSymbols(catalog, "nato").map((entry) => entry.id), ["friendly", "hostile"]);
  assert.equal(searchSymbols(catalog, "").length, catalog.length);
  assert.equal(searchSymbols(catalog, "zzz").length, 0);
});

test("recent symbols are unique, bounded and drop unknown ids", () => {
  let recent = [];
  for (const id of ["medic", "vehicle", "medic", "marker", "hostile", "friendly"]) {
    recent = pushRecentSymbol(recent, id);
  }
  assert.deepEqual(recent, ["friendly", "hostile", "marker", "medic"]);
  assert.deepEqual(
    recentSymbols(["gone", "medic", "medic", "vehicle"], catalog).map((entry) => entry.id),
    ["medic", "vehicle"],
  );
});

test("auto names use the label and the next free number", () => {
  const fire = symbol("fire", "Fire", "incident");
  const markers = [{ name: "Fire 1" }, { name: "Fire 3" }, { name: "Firebreak 9" }, { name: "Medic 4" }];
  assert.equal(autoMarkerName(fire, markers), "Fire 4");
  assert.equal(autoMarkerName(symbol("group", "Group / Community", "people"), []), "Group / Community 1");
});

test("age labels stay short", () => {
  const now = 10 * 24 * 3_600_000;
  assert.equal(markerAgeLabel(now - 30_000, now), "just now");
  assert.equal(markerAgeLabel(now - 5 * 60_000, now), "5 min ago");
  assert.equal(markerAgeLabel(now - 3 * 3_600_000, now), "3 h ago");
  assert.equal(markerAgeLabel(now - 3 * 24 * 3_600_000, now), "3 d ago");
});
