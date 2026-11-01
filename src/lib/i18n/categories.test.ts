import { describe, it, expect } from "vitest";
import zhTW from "./locales/zh-TW.json";
import zhCN from "./locales/zh-CN.json";
import en from "./locales/en.json";
import vi from "./locales/vi.json";
import th from "./locales/th.json";

const locales = { "zh-CN": zhCN, en, vi, th };
const groups = ["categories", "report"] as const;

function params(value: string): string[] {
  return [...(value.match(/\{(\w+)\}/g) ?? [])].sort();
}

describe("category and report strings", () => {
  for (const group of groups) {
    const base: Record<string, string> = zhTW[group];

    it(`${group} has the same keys and placeholders in every locale`, () => {
      for (const [name, locale] of Object.entries(locales)) {
        const strings: Record<string, string> = locale[group];
        expect(Object.keys(strings).sort(), name).toEqual(Object.keys(base).sort());
        for (const [key, value] of Object.entries(base)) {
          expect(strings[key].trim().length, `${name}.${group}.${key}`).toBeGreaterThan(0);
          expect(params(strings[key]), `${name}.${group}.${key}`).toEqual(params(value));
        }
      }
    });
  }
});
