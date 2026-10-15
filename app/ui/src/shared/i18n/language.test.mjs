import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createInstance } from "i18next";
import { systemLanguage } from "./language.ts";

test("detects Simplified Chinese without matching Traditional Chinese locales", () => {
  for (const locale of ["zh", "zh-CN", "zh-SG", "zh-Hans", "zh-Hans-CN", "ZH_cn"]) {
    assert.equal(systemLanguage(locale), "zh-Hans", locale);
  }
  for (const locale of ["en-US", "fr-FR", "zh-TW", "zh-HK", "zh-MO", "zh-Hant", "zh-Hant-CN"]) {
    assert.equal(systemLanguage(locale), "en", locale);
  }
  assert.equal(systemLanguage("ru-RU"), "ru");
});

test("i18next renders Chinese plurals and can switch back to English", async () => {
  const load = async (locale) => JSON.parse(await readFile(new URL(`./locales/${locale}.json`, import.meta.url), "utf8"));
  const i18n = createInstance();
  await i18n.init({
    resources: { "zh-Hans": { translation: await load("zh-Hans") }, en: { translation: await load("en") } },
    lng: "zh-Hans",
    fallbackLng: "en",
    interpolation: { escapeValue: false },
  });
  assert.equal(i18n.t("settings.language"), "语言");
  for (const count of [0, 1, 2, 10]) {
    assert.equal(i18n.t("library.count", { count }), `${count} 个游戏`);
  }
  assert.equal(i18n.t("buildTask.running", { title: "测试游戏" }), "正在构建《测试游戏》");
  await i18n.changeLanguage("en");
  assert.equal(i18n.t("settings.language"), "Language");
});

test("Chinese translations cover English keys and preserve interpolation and markup", async () => {
  const flatten = (value, prefix = "") => Object.entries(value).flatMap(([key, entry]) =>
    typeof entry === "string" ? [[`${prefix}${key}`, entry]] : flatten(entry, `${prefix}${key}.`));
  const load = async (locale) => new Map(flatten(JSON.parse(await readFile(new URL(`./locales/${locale}.json`, import.meta.url), "utf8"))));
  const en = await load("en");
  const zh = await load("zh-Hans");
  assert.deepEqual([...zh.keys()].sort(), [...en.keys()].sort());
  const tokens = (value, pattern) => [...value.matchAll(pattern)].map((match) => match[0]).sort();
  for (const [key, value] of en) {
    const translated = zh.get(key);
    assert.ok(translated.trim(), key);
    assert.deepEqual(tokens(translated, /{{.*?}}/g), tokens(value, /{{.*?}}/g), key);
    assert.deepEqual(tokens(translated, /<\/?[a-z]+>/g), tokens(value, /<\/?[a-z]+>/g), key);
  }
});
