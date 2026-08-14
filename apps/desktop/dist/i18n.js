(function () {
  const DEFAULT_LOCALE = "en";
  const SUPPORTED_LOCALES = new Set(["en", "zh-CN"]);
  let locale = DEFAULT_LOCALE;
  let messages = {};

  function normalizeLocale(value) {
    if (SUPPORTED_LOCALES.has(value)) return value;
    return String(value || "").toLowerCase().startsWith("zh") ? "zh-CN" : DEFAULT_LOCALE;
  }

  function interpolate(template, values = {}) {
    return String(template).replace(/\{(\w+)\}/g, (_, key) => values[key] ?? `{${key}}`);
  }

  function t(key, values) {
    return interpolate(messages[key] || key, values);
  }

  function apply(root = document) {
    root.querySelectorAll("[data-i18n]").forEach((node) => {
      node.textContent = t(node.dataset.i18n);
    });
    ["placeholder", "aria-label", "title"].forEach((attribute) => {
      const dataAttribute = `data-i18n-${attribute}`;
      root.querySelectorAll(`[${dataAttribute}]`).forEach((node) => {
        node.setAttribute(attribute, t(node.getAttribute(dataAttribute)));
      });
    });
    document.documentElement.lang = locale;
  }

  async function fetchMessages(targetLocale) {
    const response = await fetch(`i18n/${targetLocale}.json`);
    if (!response.ok) throw new Error(`Cannot load language resources (${response.status})`);
    return response.json();
  }

  async function load(nextLocale = DEFAULT_LOCALE) {
    const normalized = normalizeLocale(nextLocale);
    const fallback = await fetchMessages(DEFAULT_LOCALE);
    messages = normalized === DEFAULT_LOCALE
      ? fallback
      : { ...fallback, ...await fetchMessages(normalized) };
    locale = normalized;
    apply();
    return locale;
  }

  window.LynkoI18n = {
    apply,
    getLocale: () => locale,
    load,
    t
  };
})();
