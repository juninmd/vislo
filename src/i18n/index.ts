import { type LangCode, format, type Params, resolveLanguage } from "./core";
import { type Dict, en, type Key } from "./en";

export { AUTO, LOCALES, type LangCode } from "./core";
export type { Key } from "./en";

// One chunk per language, fetched only when that language is chosen: English users load nothing extra.
const loaders: Record<Exclude<LangCode, "en">, () => Promise<{ default: Dict }>> = {
  "pt-BR": () => import("./pt-BR"),
  es: () => import("./es"),
  fr: () => import("./fr"),
  de: () => import("./de"),
  it: () => import("./it"),
  ru: () => import("./ru"),
  tr: () => import("./tr"),
  ja: () => import("./ja"),
  ko: () => import("./ko"),
  "zh-CN": () => import("./zh-CN"),
  "zh-TW": () => import("./zh-TW"),
  pl: () => import("./pl"),
  nl: () => import("./nl"),
};

let dict: Dict = en;
let code: LangCode = "en";
const listeners = new Set<() => void>();

export const currentLanguage = (): LangCode => code;

/** Translates `key`; `{name}` placeholders are filled from `params`. */
export function t(key: Key, params?: Params): string {
  return format(dict[key] ?? en[key], params);
}

/** Runs `fn` after every language change (re-render what was built from translated text). */
export const onLanguageChange = (fn: () => void): void => void listeners.add(fn);

/** Switches to `pref` (`auto` follows the system), updates the static markup and notifies listeners. */
export async function setLanguage(pref: string, root: ParentNode = document): Promise<LangCode> {
  const next = resolveLanguage(pref, typeof navigator === "undefined" ? [] : navigator.languages);
  if (next !== code || dict === en) {
    try {
      dict = next === "en" ? en : (await loaders[next]()).default;
      code = next;
    } catch (err) {
      console.error(`vislo: cannot load language ${next}`, err); // stay on the previous language rather than a half-translated UI
    }
  }
  if (typeof document !== "undefined") document.documentElement.lang = code;
  applyStatic(root);
  for (const fn of listeners) fn();
  return code;
}

/** Fills the markup declared in index.html: data-i18n (text), data-i18n-tip (title + aria-label), data-i18n-aria. */
export function applyStatic(root: ParentNode = document): void {
  for (const el of root.querySelectorAll<HTMLElement>("[data-i18n]")) {
    const text = t(el.dataset.i18n as Key, { app: "{app}" });
    if (!text.includes("{app}")) {
      el.textContent = text;
      continue;
    }
    // The product name is bolded wherever the language puts it.
    const [before, after] = text.split("{app}");
    const brand = document.createElement("b");
    brand.textContent = "Vislo";
    el.replaceChildren(before, brand, after);
  }
  for (const el of root.querySelectorAll<HTMLElement>("[data-i18n-tip]")) {
    const label = t(el.dataset.i18nTip as Key);
    const keys = el.dataset.keys;
    el.setAttribute("aria-label", label);
    el.title = keys ? `${label} (${keys})` : label;
  }
  for (const el of root.querySelectorAll<HTMLElement>("[data-i18n-aria]")) {
    el.setAttribute("aria-label", t(el.dataset.i18nAria as Key));
  }
}

export { resolveLanguage };
