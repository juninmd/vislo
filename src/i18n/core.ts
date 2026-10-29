import type { Key } from "./en";

/** Languages offered in the settings panel, named in their own language so they stay findable. */
export const LOCALES = [
  { code: "en", name: "English" },
  { code: "pt-BR", name: "Português (Brasil)" },
  { code: "es", name: "Español" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "it", name: "Italiano" },
  { code: "ru", name: "Русский" },
  { code: "tr", name: "Türkçe" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "zh-CN", name: "简体中文" },
  { code: "zh-TW", name: "繁體中文" },
  { code: "pl", name: "Polski" },
  { code: "nl", name: "Nederlands" },
] as const;

export type LangCode = (typeof LOCALES)[number]["code"];
export const AUTO = "auto";

const known = new Set<string>(LOCALES.map((l) => l.code));

/** The language for a setting (`auto` or a code) given the system's preferred languages; unknown means English. */
export function resolveLanguage(pref: string, system: readonly string[]): LangCode {
  if (known.has(pref)) return pref as LangCode;
  for (const tag of system) {
    const lower = tag.toLowerCase();
    if (known.has(tag)) return tag as LangCode;
    if (lower.startsWith("zh")) return /^zh-(tw|hk|mo)|hant/.test(lower) ? "zh-TW" : "zh-CN";
    if (lower.startsWith("pt")) return "pt-BR";
    const base = lower.split("-")[0];
    const hit = LOCALES.find((l) => l.code === base);
    if (hit) return hit.code;
  }
  return "en";
}

/** Fills `{name}` placeholders; a missing parameter stays visible instead of printing "undefined". */
export function format(template: string, params: Record<string, string | number> = {}): string {
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

export const placeholders = (s: string): string[] => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

export type Params = Record<string, string | number>;
export type Translate = (key: Key, params?: Params) => string;
