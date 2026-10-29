import { describe, expect, it } from "vitest";
import { format, LOCALES, placeholders, resolveLanguage } from "./core";
import { en } from "./en";

// Eagerly import every language so a missing key fails the build, not a user's screen.
const modules = import.meta.glob<{ default: Record<string, string> }>("./*.ts", { eager: true });
const dicts = Object.fromEntries(
  Object.entries(modules)
    .filter(([path]) => !/\/(core|en|index|i18n\.test)\.ts$/.test(path))
    .map(([path, mod]) => [path.replace("./", "").replace(".ts", ""), mod.default]),
);

describe("dictionaries", () => {
  it("has a dictionary for every offered language and no strays", () => {
    const offered = LOCALES.map((l) => l.code).filter((c) => c !== "en");
    expect(Object.keys(dicts).sort()).toEqual([...offered].sort());
  });

  for (const code of Object.keys(dicts)) {
    it(`${code} defines exactly the English keys`, () => {
      expect(Object.keys(dicts[code]).sort()).toEqual(Object.keys(en).sort());
    });

    it(`${code} has no empty value and keeps every placeholder`, () => {
      for (const [key, english] of Object.entries(en)) {
        const text = dicts[code][key];
        expect(text.trim(), `${code}:${key}`).not.toBe("");
        expect(placeholders(text), `${code}:${key} placeholders`).toEqual(placeholders(english));
      }
    });
  }

  it("still gives translators the shortcut-free titles (shortcuts are appended in markup)", () => {
    for (const [key, text] of Object.entries(en)) {
      if (key.startsWith("hud.")) expect(text, key).not.toMatch(/\(.*\)/);
    }
  });
});

describe("resolveLanguage", () => {
  it("honours an explicit code and ignores the system", () => {
    expect(resolveLanguage("ja", ["de-DE"])).toBe("ja");
  });

  it("follows the system's order of preference for auto", () => {
    expect(resolveLanguage("auto", ["sv-SE", "es-MX", "fr"])).toBe("es");
    expect(resolveLanguage("auto", ["pt-PT"])).toBe("pt-BR");
  });

  it("separates Simplified and Traditional Chinese", () => {
    for (const tag of ["zh-TW", "zh-HK", "zh-Hant-TW", "zh-MO"]) expect(resolveLanguage("auto", [tag])).toBe("zh-TW");
    for (const tag of ["zh", "zh-CN", "zh-Hans", "zh-SG"]) expect(resolveLanguage("auto", [tag])).toBe("zh-CN");
  });

  it("falls back to English for unknown or empty input", () => {
    expect(resolveLanguage("auto", [])).toBe("en");
    expect(resolveLanguage("auto", ["sv-SE"])).toBe("en");
    expect(resolveLanguage("klingon", ["xx"])).toBe("en");
  });
});

describe("format", () => {
  it("fills placeholders and leaves unknown ones visible", () => {
    expect(format("Vislo {version} is out", { version: "1.2.3" })).toBe("Vislo 1.2.3 is out");
    expect(format("{a} and {b}", { a: 1 })).toBe("1 and {b}");
    expect(format("no params")).toBe("no params");
  });
});
