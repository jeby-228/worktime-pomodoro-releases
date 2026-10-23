import { writable, derived, get } from 'svelte/store';
import type { Locale } from './config';
import { defaultLocale, dateFormats, timeFormats } from './config';

// Import all locale files
import zhTW from './locales/zh-TW.json';
import zhCN from './locales/zh-CN.json';
import en from './locales/en.json';
import vi from './locales/vi.json';
import th from './locales/th.json';

const translations: Record<Locale, typeof zhTW> = {
  'zh-TW': zhTW,
  'zh-CN': zhCN,
  'en': en,
  'vi': vi,
  'th': th
};

// Get system locale
function getSystemLocale(): Locale {
  if (typeof window === 'undefined') return defaultLocale;
  
  const lang = navigator.language || (navigator as any).userLanguage;
  
  if (lang.startsWith('zh-TW') || lang.startsWith('zh-Hant')) return 'zh-TW';
  if (lang.startsWith('zh-CN') || lang.startsWith('zh-Hans')) return 'zh-CN';
  if (lang.startsWith('vi')) return 'vi';
  if (lang.startsWith('th')) return 'th';
  if (lang.startsWith('en')) return 'en';
  
  return defaultLocale;
}

// Load saved locale or use system default
function loadLocale(): Locale {
  if (typeof window === 'undefined') return defaultLocale;
  
  const saved = localStorage.getItem('locale');
  if (saved === 'system') {
    return getSystemLocale();
  }
  if (saved && saved in translations) {
    return saved as Locale;
  }
  return getSystemLocale();
}

// Locale store
export const locale = writable<Locale>(loadLocale());

// Subscribe to save locale changes
if (typeof window !== 'undefined') {
  locale.subscribe(value => {
    // Note: we save the actual locale, not 'system'
    // The settings UI will handle the 'system' option separately
    localStorage.setItem('locale', value);
    
    // Update HTML lang attribute
    document.documentElement.lang = value;
  });
}

// Translation function
export const t = derived(locale, ($locale) => {
  return (key: string, params?: Record<string, string | number>): string => {
    const keys = key.split('.');
    let value: any = translations[$locale];
    
    for (const k of keys) {
      if (value && typeof value === 'object' && k in value) {
        value = value[k];
      } else {
        console.warn(`Translation key not found: ${key}`);
        return key;
      }
    }
    
    if (typeof value !== 'string') {
      console.warn(`Translation key is not a string: ${key}`);
      return key;
    }
    
    // Replace parameters
    if (params) {
      return value.replace(/\{(\w+)\}/g, (_, paramKey) => {
        return String(params[paramKey] ?? `{${paramKey}}`);
      });
    }
    
    return value;
  };
});

// Format date
export function formatDate(date: Date, localeOverride?: Locale): string {
  const currentLocale = localeOverride || get(locale);
  const format = dateFormats[currentLocale];
  
  return new Intl.DateTimeFormat(currentLocale, format).format(date);
}

// Format time
export function formatTime(date: Date, localeOverride?: Locale): string {
  const currentLocale = localeOverride || get(locale);
  const format = timeFormats[currentLocale];
  
  return new Intl.DateTimeFormat(currentLocale, format).format(date);
}

// Format datetime
export function formatDateTime(date: Date, localeOverride?: Locale): string {
  const currentLocale = localeOverride || get(locale);
  
  return new Intl.DateTimeFormat(currentLocale, {
    ...dateFormats[currentLocale],
    ...timeFormats[currentLocale]
  }).format(date);
}

// Set locale
export function setLocale(newLocale: Locale | 'system') {
  if (newLocale === 'system') {
    locale.set(getSystemLocale());
    localStorage.setItem('locale_preference', 'system');
  } else {
    locale.set(newLocale);
    localStorage.setItem('locale_preference', newLocale);
  }
}

// Get current locale preference (including 'system' option)
export function getLocalePreference(): Locale | 'system' {
  if (typeof window === 'undefined') return defaultLocale;
  return (localStorage.getItem('locale_preference') as Locale | 'system') || 'system';
}
