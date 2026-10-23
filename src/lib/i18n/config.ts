export const locales = {
  "zh-TW": "繁體中文",
  "zh-CN": "简体中文",
  en: "English",
  vi: "Tiếng Việt",
  th: "ภาษาไทย",
} as const;

export type Locale = keyof typeof locales;

export const defaultLocale: Locale = "zh-TW";

// Date/time format patterns per locale
export const dateFormats: Record<Locale, Intl.DateTimeFormatOptions> = {
  "zh-TW": { year: "numeric", month: "2-digit", day: "2-digit" },
  "zh-CN": { year: "numeric", month: "2-digit", day: "2-digit" },
  en: { year: "numeric", month: "short", day: "numeric" },
  vi: { year: "numeric", month: "2-digit", day: "2-digit" },
  th: { year: "numeric", month: "2-digit", day: "2-digit" }, // Gregorian calendar
};

export const timeFormats: Record<Locale, Intl.DateTimeFormatOptions> = {
  "zh-TW": { hour: "2-digit", minute: "2-digit", hour12: false },
  "zh-CN": { hour: "2-digit", minute: "2-digit", hour12: false },
  en: { hour: "2-digit", minute: "2-digit", hour12: true },
  vi: { hour: "2-digit", minute: "2-digit", hour12: false },
  th: { hour: "2-digit", minute: "2-digit", hour12: false },
};
