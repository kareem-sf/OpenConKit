import { describe, expect, it } from "vitest";

import arCommon from "../src/locales/ar/common.json";
import enCommon from "../src/locales/en/common.json";
import { createI18n, directionOf, isRtl } from "../src/index";

type JsonObject = { [key: string]: string | JsonObject };

function flattenKeys(value: JsonObject, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof child === "string" ? [path] : flattenKeys(child, path);
  });
}

describe("locale key parity", () => {
  it("ar contains exactly the same keys as en", () => {
    const enKeys = flattenKeys(enCommon as JsonObject).sort();
    const arKeys = flattenKeys(arCommon as JsonObject).sort();
    expect(arKeys).toEqual(enKeys);
  });

  it("no locale value is empty", () => {
    for (const locale of [enCommon, arCommon] as JsonObject[]) {
      for (const key of flattenKeys(locale)) {
        const value = key
          .split(".")
          .reduce<string | JsonObject>(
            (node, segment) => (typeof node === "string" ? node : (node[segment] ?? "")),
            locale,
          );
        expect(typeof value === "string" && value.trim().length > 0, key).toBe(true);
      }
    }
  });
});

describe("direction helpers", () => {
  it("marks ar as RTL and en as LTR", () => {
    expect(isRtl("ar")).toBe(true);
    expect(isRtl("ar-EG")).toBe(true);
    expect(isRtl("en")).toBe(false);
    expect(directionOf("ar")).toBe("rtl");
    expect(directionOf("en-US")).toBe("ltr");
  });
});

describe("createI18n", () => {
  it("resolves tagline in en and ar", () => {
    const en = createI18n("en");
    expect(en.t("app.tagline")).toBe("The open-source toolkit for construction professionals.");
    const ar = createI18n("ar");
    expect(ar.t("app.tagline")).toBe("مجموعة الأدوات مفتوحة المصدر لمهنيي قطاع الإنشاءات.");
  });

  it("falls back to en for unsupported languages", () => {
    const instance = createI18n("fr");
    expect(instance.t("nav.home")).toBe("Home");
  });
});
