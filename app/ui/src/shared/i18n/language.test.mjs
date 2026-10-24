import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createInstance } from "i18next";
import { systemLanguage } from "./language.ts";

const LOCALES = ["en", "ru", "zh-Hans", "ja", "ko", "zh-Hant", "fr", "fr-CA", "de", "it", "es", "es-419", "nl", "pt", "pt-BR"];
const i18nCode = (locale) => (locale === "pt" ? "pt-PT" : locale);

const load = async (locale) => JSON.parse(await readFile(new URL(`./locales/${locale}.json`, import.meta.url), "utf8"));
const flatten = (value, prefix = "") => Object.entries(value).flatMap(([key, entry]) =>
  typeof entry === "string" ? [[`${prefix}${key}`, entry]] : flatten(entry, `${prefix}${key}.`));
const PLURAL = /_(zero|one|two|few|many|other)$/;

test("maps Windows locales to the interface languages", () => {
  const cases = {
    "zh-Hans": ["zh", "zh-CN", "zh-SG", "zh-Hans", "zh-Hans-CN", "ZH_cn", "zh-Hans-HK"],
    "zh-Hant": ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "zh-Hant-CN", "zh-Hant-TW", "ZH_tw"],
    ja: ["ja", "ja-JP"],
    ko: ["ko", "ko-KR"],
    fr: ["fr", "fr-FR", "fr-BE", "fr-CH"],
    "fr-CA": ["fr-CA", "FR_ca"],
    de: ["de", "de-DE", "de-AT", "de-CH"],
    it: ["it", "it-IT"],
    es: ["es", "es-ES", "es-ES-u-co-trad"],
    "es-419": ["es-419", "es-MX", "es-AR", "es-US", "es-CO"],
    nl: ["nl", "nl-NL", "nl-BE"],
    pt: ["pt-PT", "pt-AO", "pt-MZ"],
    "pt-BR": ["pt", "pt-BR", "PT_br"],
    ru: ["ru", "ru-RU", "ru-UA"],
    en: ["en", "en-US", "en-GB", "en-AU", "pl-PL", "uk-UA", "", "x", "zz-ZZ"],
  };
  for (const [expected, locales] of Object.entries(cases)) {
    for (const locale of locales) {
      assert.equal(systemLanguage(locale), expected, locale);
    }
  }
});

test("every translation covers the English keys with its own plural forms", async () => {
  const en = new Map(flatten(await load("en")));
  const bases = new Set([...en.keys()].filter((key) => PLURAL.test(key)).map((key) => key.replace(PLURAL, "")));
  const tokens = (value, pattern) => [...value.matchAll(pattern)].map((match) => match[0]).sort();
  for (const locale of LOCALES) {
    const translated = new Map(flatten(await load(locale)));
    const categories = new Intl.PluralRules(i18nCode(locale)).resolvedOptions().pluralCategories;
    const expected = [...en.keys()].filter((key) => !PLURAL.test(key));
    for (const base of bases) {
      expected.push(...categories.map((category) => `${base}_${category}`));
    }
    assert.deepEqual([...translated.keys()].sort(), expected.sort(), locale);
    for (const [key, value] of translated) {
      const source = en.get(key) ?? en.get(`${key.replace(PLURAL, "")}_other`);
      assert.ok(value.trim(), `${locale} ${key}`);
      assert.ok(!/[\t\n\r]/.test(value), `${locale} ${key}`);
      assert.deepEqual(tokens(value, /{{.*?}}/g), tokens(source, /{{.*?}}/g), `${locale} ${key}`);
      assert.deepEqual(tokens(value, /<\/?[a-z]+>/g), tokens(source, /<\/?[a-z]+>/g), `${locale} ${key}`);
    }
  }
});

test("i18next renders every language with plurals and no English fallback", async () => {
  const resources = {};
  for (const locale of LOCALES) {
    resources[i18nCode(locale)] = { translation: await load(locale) };
  }
  const i18n = createInstance();
  await i18n.init({ resources, lng: "en", fallbackLng: "en", interpolation: { escapeValue: false } });
  for (const code of LOCALES.map(i18nCode)) {
    await i18n.changeLanguage(code);
    assert.equal(i18n.language, code);
    const rules = new Intl.PluralRules(code);
    for (const count of [0, 1, 2, 5, 21, 1000000]) {
      const own = resources[code].translation.library[`count_${rules.select(count)}`];
      assert.equal(i18n.t("library.count", { count }), own.replaceAll("{{count}}", String(count)), `${code} ${count}`);
    }
  }
  await i18n.changeLanguage("pt-PT");
  assert.match(i18n.t("library.count", { count: 0 }), /^0 \S+s\b/);
  await i18n.changeLanguage("zh-Hans");
  assert.equal(i18n.t("settings.language"), "语言");
  assert.equal(i18n.t("buildTask.running", { title: "测试游戏" }), "正在构建《测试游戏》");
});
