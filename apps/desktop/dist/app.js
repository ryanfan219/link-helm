const invoke = window.__TAURI_INTERNALS__?.invoke;
const state = { snapshot: null, selectedRuleId: null, locale: "en" };

const $ = (id) => document.getElementById(id);
const escapeHtml = (value) => String(value ?? "").replace(/[&<>'"]/g, (char) => ({"&":"&amp;","<":"&lt;",">":"&gt;","'":"&#39;",'"':"&quot;"}[char]));
const t = (key, values) => window.LinkHelmI18n.t(key, values);

function setInteractiveReady(ready) {
  document.querySelectorAll(".content button, .content input, .content select, .content textarea").forEach((control) => {
    control.disabled = !ready;
  });
  document.querySelector("main")?.setAttribute("aria-busy", String(!ready));
}

function requireSnapshot() {
  if (!state.snapshot) throw new Error(t("error.stillLoading"));
  return state.snapshot;
}

async function call(command, args = {}) {
  if (!invoke) throw new Error(t("error.ipcUnavailable"));
  return invoke(command, args);
}

function notify(message, error = false) {
  const notice = $("notice");
  notice.textContent = message;
  notice.classList.toggle("error", error);
  notice.hidden = false;
  window.setTimeout(() => { notice.hidden = true; }, 4500);
}

function allProfiles() {
  return (state.snapshot?.browsers ?? []).flatMap((browser) => browser.profiles.map((profile) => ({ browser, profile })));
}

function encodeProfileValue(browserId, profileId) {
  return JSON.stringify([browserId, profileId]);
}

function decodeProfileValue(value) {
  try {
    const decoded = JSON.parse(value);
    return Array.isArray(decoded) && decoded.length === 2 ? decoded : [null, null];
  } catch (_) {
    return [null, null];
  }
}

function matcherValues(value) {
  if (Array.isArray(value)) return value;
  return value ? [value] : [];
}

function parseMatcherInput(value) {
  return [...new Set(value.split(/[\n,]+/).map((item) => item.trim()).filter(Boolean))];
}

function matcherPayload(value) {
  const values = parseMatcherInput(value);
  if (!values.length) return null;
  return values.length === 1 ? values[0] : values;
}

function profileOptions(selected = "") {
  const profiles = allProfiles();
  if (!profiles.length) return `<option value="">${escapeHtml(t("browser.noProfiles"))}</option>`;
  return profiles.map(({browser, profile}) => {
    const value = encodeProfileValue(browser.descriptor.id, profile.profile_id);
    return `<option value="${escapeHtml(value)}" ${value === selected ? "selected" : ""}>${escapeHtml(browser.descriptor.display_name)} — ${escapeHtml(profile.display_name)}</option>`;
  }).join("");
}

function ruleProfileOptions(selected = "") {
  const profileEntries = allProfiles().map(({browser, profile}) => {
    const value = encodeProfileValue(browser.descriptor.id, profile.profile_id);
    return `<option value="${escapeHtml(value)}" ${value === selected ? "selected" : ""}>${escapeHtml(browser.descriptor.display_name)} — ${escapeHtml(profile.display_name)}</option>`;
  });
  const browserDefaults = (state.snapshot?.browsers ?? [])
    .filter((browser) => browser.installed && (!browser.profiles.length || encodeProfileValue(browser.descriptor.id, null) === selected))
    .map((browser) => {
      const value = encodeProfileValue(browser.descriptor.id, null);
      return `<option value="${escapeHtml(value)}" ${value === selected ? "selected" : ""}>${escapeHtml(browser.descriptor.display_name)} — ${escapeHtml(t("browser.defaultBehavior"))}</option>`;
    });
  const options = [...profileEntries, ...browserDefaults];
  return options.length ? options.join("") : `<option value="">${escapeHtml(t("browser.noProfiles"))}</option>`;
}

function browserOptions(selected = "") {
  return (state.snapshot?.browsers ?? []).filter((browser) => browser.installed).map((browser) => `<option value="${escapeHtml(browser.descriptor.id)}" ${browser.descriptor.id === selected ? "selected" : ""}>${escapeHtml(browser.descriptor.display_name)}</option>`).join("");
}

function render() {
  const snapshot = state.snapshot;
  if (!snapshot) return;
  $("config-error").hidden = !snapshot.config_error;
  $("config-error").textContent = snapshot.config_error ? t("error.safeMode", { error: snapshot.config_error }) : "";
  $("diagnostics-error").hidden = !snapshot.diagnostics_error;
  $("diagnostics-error").textContent = snapshot.diagnostics_error || "";
  if (document.activeElement !== $("diagnostics-limit")) $("diagnostics-limit").value = snapshot.diagnostics_limit;
  $("paused").checked = snapshot.paused;
  $("ask-next").checked = snapshot.ask_next;
  $("routing-dot").classList.toggle("paused", snapshot.paused);
  $("routing-label").textContent = t(snapshot.paused ? "status.routingPaused" : "status.routingActive");
  $("default-status").textContent = snapshot.system.is_default_browser ? t("status.default") : t("status.current", { value: snapshot.system.http_handler || t("status.unknown") });
  $("default-status").className = `badge ${snapshot.system.is_default_browser ? "success" : "neutral"}`;
  $("set-default").disabled = snapshot.system.is_default_browser;
  $("rules-inactive").hidden = snapshot.system.is_default_browser;
  $("accessibility-setting").hidden = !snapshot.system.accessibility_required;
  $("accessibility-status").textContent = t(snapshot.system.accessibility_trusted ? "status.granted" : "status.notGranted");
  $("accessibility-status").className = `badge ${snapshot.system.accessibility_trusted ? "success" : "neutral"}`;
  $("open-accessibility").hidden = snapshot.system.accessibility_trusted;
  $("test-profile").innerHTML = profileOptions($("test-profile").value);
  $("rule-profile").innerHTML = ruleProfileOptions($("rule-profile").value);
  $("rule-browser").innerHTML = browserOptions($("rule-browser").value);
  renderBrowsers();
  renderRules();
  renderDiagnostics();
}

function renderBrowsers() {
  const list = $("browser-list");
  list.innerHTML = state.snapshot.browsers.map((browser) => {
    const capabilities = Object.entries(browser.capabilities).filter(([,value]) => value).map(([key]) => `<span class="capability">${escapeHtml(t(`capability.${key}`))}</span>`).join("");
    const profiles = browser.profiles.length ? browser.profiles.map((profile) => `<div class="profile-row"><strong>${escapeHtml(profile.display_name)}</strong><span class="profile-id">${escapeHtml(profile.profile_id)}</span></div>`).join("") : `<div class="empty">${browser.installed ? escapeHtml(browser.error || t("browser.noProfilesDefaultAvailable")) : escapeHtml(t("browser.notInstalled"))}</div>`;
    return `<article class="browser-item"><div class="browser-head"><div><h2>${escapeHtml(browser.descriptor.display_name)}</h2><div class="browser-id">${escapeHtml(browser.descriptor.id)}</div></div><span class="badge ${browser.installed ? "success" : "missing"}">${escapeHtml(t(browser.installed ? "status.installed" : "status.missing"))}</span></div><div class="capabilities">${capabilities}</div><div class="profile-list">${profiles}</div></article>`;
  }).join("");
}

function ruleSummary(rule) {
  const sourceApps = matcherValues(rule.matcher.source_app).join(", ");
  const domains = matcherValues(rule.matcher.domain).join(", ");
  const match = [sourceApps, domains].filter(Boolean).join(" + ") || t("rules.allAppsAndLinks");
  return { title: rule.name || rule.id, subtitle: match };
}

function renderRules() {
  const rules = state.snapshot.config.rules;
  $("rule-list").innerHTML = rules.length ? rules.map((rule) => { const summary = ruleSummary(rule); return `<button class="rule-item ${rule.id === state.selectedRuleId ? "active" : ""}" data-rule-id="${escapeHtml(rule.id)}"><span class="rule-item-copy"><strong>${escapeHtml(summary.title)}</strong><small>${escapeHtml(summary.subtitle)}</small></span><span class="badge ${rule.enabled ? "success" : "neutral"}">${escapeHtml(t(rule.enabled ? "status.enabled" : "status.disabled"))}</span></button>`; }).join("") : `<div class="empty">${escapeHtml(t("rules.empty"))}</div>`;
  document.querySelectorAll(".rule-item").forEach((button) => button.addEventListener("click", () => selectRule(button.dataset.ruleId)));
}

function renderDiagnostics() {
  const events = state.snapshot.diagnostics;
  $("diagnostic-list").innerHTML = events.length ? [...events].reverse().map((event) => `<div class="diagnostic-row"><time>${new Date(event.timestamp_ms).toLocaleString(state.locale)}</time><span>${escapeHtml(event.source_app)}</span><span>${escapeHtml(event.domain)}</span><span>${escapeHtml(event.outcome)}</span></div>`).join("") : `<div class="empty">${escapeHtml(t("diagnostics.empty"))}</div>`;
}

function selectRule(id) {
  state.selectedRuleId = id;
  const rule = state.snapshot.config.rules.find((item) => item.id === id);
  if (!rule) return closeRuleDialog();
  $("editor-title").textContent = t("rules.editRule", { name: rule.name || rule.id });
  $("rule-id").value = rule.id;
  $("rule-name").value = rule.name || rule.id;
  $("rule-enabled").checked = rule.enabled;
  $("rule-source").value = matcherValues(rule.matcher.source_app).join("\n");
  $("rule-domain").value = matcherValues(rule.matcher.domain).join("\n");
  $("rule-mode").value = rule.target.mode === "browser_default" ? "specified_profile" : rule.target.mode;
  $("rule-enforcement").value = rule.enforcement;
  $("rule-fallback").value = rule.fallback_scope;
  $("rule-unavailable").value = rule.unavailable_action;
  const profileValue = rule.target.browser_id && (rule.target.profile_id || rule.target.mode === "browser_default") ? encodeProfileValue(rule.target.browser_id, rule.target.profile_id || null) : "";
  $("rule-profile").innerHTML = ruleProfileOptions(profileValue);
  $("rule-browser").innerHTML = browserOptions(rule.target.browser_id || "");
  updateModeFields();
  $("delete-rule").hidden = false;
  $("save-rule").textContent = t("action.saveChanges");
  renderRules();
  $("rule-dialog").showModal();
  $("rule-name").focus();
}

function resetRuleForm() {
  state.selectedRuleId = null;
  $("rule-form").reset();
  $("rule-enabled").checked = true;
  $("rule-id").value = "";
  $("rule-name").value = "";
  $("editor-title").textContent = t("rules.newRule");
  $("rule-profile").innerHTML = ruleProfileOptions();
  $("rule-browser").innerHTML = browserOptions();
  $("delete-rule").hidden = true;
  $("save-rule").textContent = t("action.createRule");
  updateModeFields();
  renderRules();
}

function openNewRule() {
  resetRuleForm();
  $("rule-dialog").showModal();
  $("rule-name").focus();
}

function closeRuleDialog() {
  if ($("rule-dialog").open) $("rule-dialog").close();
  state.selectedRuleId = null;
  renderRules();
}

function updateModeFields() {
  const mode = $("rule-mode").value;
  const [selectedBrowserId, selectedProfileId] = decodeProfileValue($("rule-profile").value);
  const browserDefault = mode === "specified_profile" && selectedBrowserId && !selectedProfileId;
  $("rule-profile-label").hidden = mode !== "specified_profile";
  $("rule-browser-label").hidden = mode !== "active_in_browser";
  $("policy-fields").hidden = mode === "ask" || browserDefault;
  if (mode === "ask") {
    $("rule-enforcement").value = "prefer";
    $("rule-fallback").value = "none";
    $("rule-unavailable").value = "ask";
  }
  if (browserDefault) {
    $("rule-enforcement").value = "prefer";
    $("rule-fallback").value = "none";
    $("rule-unavailable").value = "fail";
  }
  if ($("rule-enforcement").value === "force") $("rule-fallback").value = "none";
}

function formRule() {
  let mode = $("rule-mode").value;
  let browserId = null, profileId = null;
  if (mode === "specified_profile") {
    [browserId, profileId] = decodeProfileValue($("rule-profile").value);
    if (!browserId) throw new Error(t("error.selectProfile"));
    if (!profileId) mode = "browser_default";
  }
  if (mode === "active_in_browser") {
    browserId = $("rule-browser").value;
    if (!browserId) throw new Error(t("error.selectBrowser"));
  }
  const name = $("rule-name").value.trim();
  if (!name) throw new Error(t("error.enterRuleName"));
  return {
    id: state.selectedRuleId || `rule-${Date.now()}`,
    name,
    enabled: $("rule-enabled").checked,
    order: state.selectedRuleId ? state.snapshot.config.rules.findIndex((rule) => rule.id === state.selectedRuleId) : state.snapshot.config.rules.length,
    matcher: { source_app: matcherPayload($("rule-source").value), domain: matcherPayload($("rule-domain").value) },
    target: { mode, browser_id: browserId || null, profile_id: profileId || null },
    enforcement: mode === "ask" || mode === "browser_default" ? "prefer" : $("rule-enforcement").value,
    fallback_scope: mode === "ask" || mode === "browser_default" || $("rule-enforcement").value === "force" ? "none" : $("rule-fallback").value,
    unavailable_action: mode === "ask" ? "ask" : mode === "browser_default" ? "fail" : $("rule-unavailable").value
  };
}

async function saveRule(event) {
  event.preventDefault();
  try {
    const rule = formRule();
    const rules = [...state.snapshot.config.rules];
    const existing = rules.findIndex((item) => item.id === state.selectedRuleId);
    if (existing >= 0) rules[existing] = rule; else rules.push(rule);
    await call("save_config", { config: { schema_version: state.snapshot.config.schema_version, rules } });
    state.snapshot.config.rules = rules;
    render();
    closeRuleDialog();
    notify(t(existing >= 0 ? "notice.ruleUpdated" : "notice.ruleCreated"));
  } catch (error) { notify(String(error), true); }
}

async function initialize() {
  setInteractiveReady(false);
  try {
    await window.LinkHelmI18n.load("en").catch((error) => { throw new Error(`Language resources failed: ${String(error)}`); });
    const snapshot = await call("get_state").catch((error) => { throw new Error(t("error.applicationState", { error: String(error) })); });
    const locale = snapshot.locale === "en"
      ? "en"
      : await window.LinkHelmI18n.load(snapshot.locale).catch((error) => { throw new Error(`Language resources failed: ${String(error)}`); });
    state.locale = locale;
    $("language").value = locale;
    state.snapshot = snapshot;
    setInteractiveReady(true);
    render();
    resetRuleForm();
  } catch (error) {
    const message = t("error.initialize", { error: String(error) });
    notify(message === "error.initialize" ? `Link Helm could not initialize: ${String(error)}` : message, true);
  }
}

$("language").addEventListener("change", async (event) => {
  const previousLocale = state.locale;
  event.target.disabled = true;
  try {
    const locale = await window.LinkHelmI18n.load(event.target.value);
    state.locale = await call("set_locale", { locale });
    state.snapshot.locale = state.locale;
    render();
    if (state.selectedRuleId) {
      const rule = state.snapshot?.config.rules.find((item) => item.id === state.selectedRuleId);
      if (rule) {
        $("editor-title").textContent = t("rules.editRule", { name: rule.name || rule.id });
        $("save-rule").textContent = t("action.saveChanges");
      }
    }
    notify(t("notice.languageChanged"));
  } catch (error) {
    state.locale = await window.LinkHelmI18n.load(previousLocale);
    event.target.value = previousLocale;
    notify(t("error.languageChange", { error: String(error) }), true);
  } finally {
    event.target.disabled = false;
  }
});
document.querySelectorAll(".nav-item").forEach((button) => button.addEventListener("click", () => {
  document.querySelectorAll(".nav-item, .page").forEach((node) => node.classList.remove("active"));
  button.classList.add("active");
  document.querySelector(`.page[data-page="${button.dataset.page}"]`).classList.add("active");
}));
$("rule-mode").addEventListener("change", updateModeFields);
$("rule-profile").addEventListener("change", updateModeFields);
$("rule-enforcement").addEventListener("change", updateModeFields);
$("rule-form").addEventListener("submit", saveRule);
$("add-rule").addEventListener("click", openNewRule);
$("close-rule-dialog").addEventListener("click", closeRuleDialog);
$("cancel-rule-dialog").addEventListener("click", closeRuleDialog);
$("rule-dialog").addEventListener("close", () => {
  state.selectedRuleId = null;
  renderRules();
});
$("choose-source-app").addEventListener("click", async () => {
  try {
    requireSnapshot();
    const bundleId = await call("choose_source_application");
    if (bundleId) {
      const sourceApps = parseMatcherInput($("rule-source").value);
      if (!sourceApps.includes(bundleId)) sourceApps.push(bundleId);
      $("rule-source").value = sourceApps.join("\n");
    }
  } catch (error) { notify(String(error), true); }
});
$("clear-source-app").addEventListener("click", () => { $("rule-source").value = ""; });
$("delete-rule").addEventListener("click", async () => {
  const snapshot = requireSnapshot();
  if (!state.selectedRuleId) return closeRuleDialog();
  const rules = snapshot.config.rules.filter((rule) => rule.id !== state.selectedRuleId).map((rule, order) => ({...rule, order}));
  try { await call("save_config", {config:{...snapshot.config, rules}}); snapshot.config.rules = rules; closeRuleDialog(); notify(t("notice.ruleDeleted")); } catch (error) { notify(String(error), true); }
});
$("rescan").addEventListener("click", async () => { try { const snapshot = requireSnapshot(); snapshot.browsers = await call("scan_browsers"); render(); notify(t("notice.scanCompleted")); } catch (error) { notify(String(error), true); } });
$("paused").addEventListener("change", async (event) => { try { const snapshot = requireSnapshot(); await call("set_paused", {paused:event.target.checked}); snapshot.paused = event.target.checked; render(); } catch (error) { notify(String(error), true); } });
$("ask-next").addEventListener("change", async (event) => { try { const snapshot = requireSnapshot(); await call("set_ask_next", {askNext:event.target.checked}); snapshot.ask_next = event.target.checked; render(); } catch (error) { notify(String(error), true); } });
$("preview-button").addEventListener("click", async () => { try { const result = await call("preview_route", {sourceApp:$("preview-source").value, url:$("preview-url").value}); $("preview-result").textContent = t("rules.previewResult", { action: result.final_action, rule: result.matched_rule_id || t("option.fallback.none"), reason: typeof result.reason === "string" ? result.reason : JSON.stringify(result.reason) }); } catch (error) { $("preview-result").textContent = String(error); } });
$("test-open").addEventListener("click", async () => { const [browserId, profileId] = decodeProfileValue($("test-profile").value); try { requireSnapshot(); if (!browserId || !profileId) throw new Error(t("error.selectProfile")); await call("test_open", {browserId, profileId, url:$("test-url").value}); state.snapshot = await call("get_state"); render(); notify(t("notice.testOpened")); } catch (error) { notify(String(error), true); } });
$("clear-diagnostics").addEventListener("click", async () => { try { const snapshot = requireSnapshot(); await call("clear_diagnostics"); snapshot.diagnostics = []; renderDiagnostics(); } catch (error) { notify(String(error), true); } });
$("save-diagnostics-limit").addEventListener("click", async () => {
  try {
    const snapshot = requireSnapshot();
    const limit = Number($("diagnostics-limit").value);
    if (!Number.isInteger(limit) || limit < 1 || limit > 100000) throw new Error(t("error.invalidDiagnosticsLimit"));
    await call("set_diagnostics_limit", {limit});
    snapshot.diagnostics_limit = limit;
    snapshot.diagnostics = snapshot.diagnostics.slice(-limit);
    renderDiagnostics();
    notify(t("notice.diagnosticsUpdated"));
  } catch (error) { notify(String(error), true); }
});
$("set-default").addEventListener("click", async () => {
  try {
    const snapshot = requireSnapshot();
    snapshot.system = await call("set_default_browser");
    render();
    notify(t(snapshot.system.is_default_browser ? "notice.defaultBrowserSet" : "notice.defaultBrowserSelectionRequired"));
  } catch (error) { notify(String(error), true); }
});
$("open-default-settings").addEventListener("click", async () => { try { requireSnapshot(); await call("open_default_browser_settings"); notify(t("notice.defaultSettingsOpened")); } catch (error) { notify(String(error), true); } });
$("open-accessibility").addEventListener("click", async () => { try { requireSnapshot(); await call("open_accessibility_settings"); notify(t("notice.accessibilitySettingsOpened")); } catch (error) { notify(String(error), true); } });
$("config-json").addEventListener("input", () => {
  $("apply-import").disabled = true;
  $("config-preview").textContent = t("config.previewRequired");
});
$("export-config").addEventListener("click", async () => {
  try {
    requireSnapshot();
    $("config-json").value = await call("export_config");
    $("apply-import").disabled = true;
    $("config-preview").textContent = t("config.exported");
  } catch (error) { notify(String(error), true); }
});
$("preview-import").addEventListener("click", async () => {
  try {
    requireSnapshot();
    const preview = await call("preview_import_config", {json:$("config-json").value});
    $("config-preview").textContent = t("config.previewSummary", { schema: preview.schema_version, rules: preview.rule_count, enabled: preview.enabled_rule_count });
    $("apply-import").disabled = false;
  } catch (error) {
    $("apply-import").disabled = true;
    $("config-preview").textContent = String(error);
  }
});
$("apply-import").addEventListener("click", async () => {
  try {
    requireSnapshot();
    await call("import_config", {json:$("config-json").value});
    state.snapshot = await call("get_state");
    state.selectedRuleId = null;
    render();
    resetRuleForm();
    $("apply-import").disabled = true;
    notify(t("notice.configurationImported"));
  } catch (error) { notify(String(error), true); }
});

window.addEventListener("unhandledrejection", (event) => {
  notify(String(event.reason || t("error.unexpectedAsync")), true);
  event.preventDefault();
});

window.addEventListener("focus", async () => {
  if (!state.snapshot) return;
  try {
    state.snapshot = await call("get_state");
    render();
  } catch (error) { notify(String(error), true); }
});

initialize();
