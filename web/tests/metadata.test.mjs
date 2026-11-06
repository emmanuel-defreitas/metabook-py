import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
const exports = {};
vm.runInNewContext(ts.transpileModule(readFileSync("src/lib/metadata.ts", "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS },
}).outputText, { exports });
const visible = exports.isVisibleMetadataValue;
test("hide absent and zero metadata including formatted units", () => {
  for (const value of [null, undefined, 0, NaN, "", "0", "0%", "0.00 ms", "0 · tokenizer", "Not provided", "Not applicable"]) {
    assert.equal(visible(value), false, String(value));
  }
});
test("preserve nonzero fractions, counts, dates and meaningful text", () => {
  for (const value of [0.1, 61, "0.25", "0.25%", "2,310", "90%", "Nov 3, 2026", "No · automatic selection"]) {
    assert.equal(visible(value), true, String(value));
  }
});
