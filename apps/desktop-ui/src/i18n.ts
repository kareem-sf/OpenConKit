import { createI18n } from "@openconkit/i18n";

/**
 * Application i18n instance. Language is detected from the browser/webview
 * locale and falls back to English for unsupported tags.
 */
const detected = typeof navigator === "undefined" ? "en" : navigator.language;

export const i18n = createI18n(detected);
