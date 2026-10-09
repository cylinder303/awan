import { test } from "node:test";
import assert from "node:assert/strict";
import ar from "../src/locales/ar.js";
import en from "../src/locales/en.js";

const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

test("Arabic and English define the same keys", () => {
  assert.deepEqual(Object.keys(ar).sort(), Object.keys(en).sort());
});

test("translations use the same placeholders", () => {
  for (const key of Object.keys(en)) {
    assert.deepEqual(placeholders(ar[key]), placeholders(en[key]), key);
  }
});

test("no translation is empty", () => {
  for (const [key, value] of [...Object.entries(ar), ...Object.entries(en)]) {
    assert.ok(value.trim().length > 0, key);
  }
});
