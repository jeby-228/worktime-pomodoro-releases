import { describe, it, expect } from "vitest";
import zhTW from "./locales/zh-TW.json";
import zhCN from "./locales/zh-CN.json";
import en from "./locales/en.json";
import vi from "./locales/vi.json";
import th from "./locales/th.json";

const locales = { "zh-TW": zhTW, "zh-CN": zhCN, en, vi, th };

function getAllKeys(obj: any, prefix = ""): string[] {
  let keys: string[] = [];

  for (const key in obj) {
    const fullKey = prefix ? `${prefix}.${key}` : key;

    if (typeof obj[key] === "object" && obj[key] !== null && !Array.isArray(obj[key])) {
      keys = keys.concat(getAllKeys(obj[key], fullKey));
    } else {
      keys.push(fullKey);
    }
  }

  return keys.sort();
}

describe("i18n Locales", () => {
  it("should have all locales with the same keys", () => {
    const baseKeys = getAllKeys(zhTW);

    for (const [localeName, localeData] of Object.entries(locales)) {
      if (localeName === "zh-TW") continue; // Skip base locale

      const localeKeys = getAllKeys(localeData);

      // Check for missing keys
      const missingKeys = baseKeys.filter((key) => !localeKeys.includes(key));
      expect(missingKeys, `${localeName} is missing keys: ${missingKeys.join(", ")}`).toEqual([]);

      // Check for extra keys
      const extraKeys = localeKeys.filter((key) => !baseKeys.includes(key));
      expect(extraKeys, `${localeName} has extra keys: ${extraKeys.join(", ")}`).toEqual([]);
    }
  });

  it("should have valid translation values (no empty strings)", () => {
    for (const [localeName, localeData] of Object.entries(locales)) {
      const keys = getAllKeys(localeData);

      for (const key of keys) {
        const value = key.split(".").reduce((obj: any, k) => obj?.[k], localeData);

        expect(
          typeof value === "string" && value.trim().length > 0,
          `${localeName}.${key} should not be empty`,
        ).toBe(true);
      }
    }
  });

  it("should have consistent parameter placeholders", () => {
    const baseKeys = getAllKeys(zhTW);

    for (const key of baseKeys) {
      const baseValue = key.split(".").reduce((obj: any, k) => obj?.[k], zhTW);

      if (typeof baseValue !== "string") continue;

      const baseParams = (baseValue.match(/\{(\w+)\}/g) || []).sort();

      for (const [localeName, localeData] of Object.entries(locales)) {
        if (localeName === "zh-TW") continue;

        const localeValue = key.split(".").reduce((obj: any, k) => obj?.[k], localeData);

        if (typeof localeValue !== "string") continue;

        const localeParams = (localeValue.match(/\{(\w+)\}/g) || []).sort();

        expect(
          localeParams,
          `${localeName}.${key} should have same parameters as base: ${baseParams.join(", ")}`,
        ).toEqual(baseParams);
      }
    }
  });
});
