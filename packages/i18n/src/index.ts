import i18next, { type i18n } from "i18next";
import { initReactI18next } from "react-i18next";

import arCommon from "./locales/ar/common.json";
import enCommon from "./locales/en/common.json";

/** Languages whose UI direction is right-to-left. */
export const RTL_LANGUAGES: readonly string[] = ["ar"];

/** Whether a language tag is right-to-left. */
export function isRtl(language: string): boolean {
  return RTL_LANGUAGES.some((rtl) => language === rtl || language.startsWith(`${rtl}-`));
}

/** BCP-47 direction for a language tag. */
export function directionOf(language: string): "rtl" | "ltr" {
  return isRtl(language) ? "rtl" : "ltr";
}

/** English resources are the source of truth for key parity (see test/). */
export const resources = {
  en: { common: enCommon },
  ar: { common: arCommon },
} as const;

/** Shape of the `common` namespace, derived from the English source. */
export type CommonResources = typeof enCommon;

/**
 * Create and initialize an i18next instance.
 *
 * @param language Initial language; falls back to `en` for unsupported tags.
 */
export function createI18n(language?: string): i18n {
  const instance = i18next.createInstance();
  void instance.use(initReactI18next).init({
    resources,
    lng: language,
    fallbackLng: "en",
    defaultNS: "common",
    ns: ["common"],
    interpolation: {
      // React already escapes interpolated values.
      escapeValue: false,
    },
    returnNull: false,
  });
  return instance;
}

export { i18next };
