const invoke = window.__TAURI_INTERNALS__?.invoke;
let pendingId = null;
const escapeHtml = (value) => String(value ?? "").replace(/[&<>'"]/g, (char) => ({"&":"&amp;","<":"&lt;",">":"&gt;","'":"&#39;",'"':"&quot;"}[char]));
const t = (key, values) => window.LynkoI18n.t(key, values);

function closeWindow() {
  window.close();
}

async function initialize() {
  try {
    const state = await invoke("get_selector_state");
    await window.LynkoI18n.load(state.locale);
    const pending = state.pending[0];
    if (!pending) {
      pendingId = null;
      return closeWindow();
    }
    pendingId = pending.id;
    document.getElementById("selector-domain").textContent = pending.domain;
    const profiles = state.browsers.flatMap((browser) => browser.profiles.map((profile) => ({browser, profile})));
    document.getElementById("selector-profiles").innerHTML = profiles.map(({browser, profile}, index) => `<button class="selector-profile" data-browser="${escapeHtml(browser.descriptor.id)}" data-profile="${escapeHtml(profile.profile_id)}" role="option"><strong>${escapeHtml(browser.descriptor.display_name)}</strong><span>${escapeHtml(profile.display_name)}</span><small>${escapeHtml(profile.profile_id)}</small><kbd>${index + 1}</kbd></button>`).join("") || `<div class="empty">${escapeHtml(t("selector.empty"))}</div>`;
    document.querySelectorAll(".selector-profile").forEach((button, index) => {
      button.addEventListener("click", () => choose(button));
      if (index === 0) button.focus();
    });
  } catch (error) {
    document.getElementById("selector-error").textContent = String(error);
  }
}

async function choose(button) {
  try {
    button.disabled = true;
    await invoke("choose_pending", {id: pendingId, browserId: button.dataset.browser, profileId: button.dataset.profile});
    await initialize();
  } catch (error) {
    button.disabled = false;
    document.getElementById("selector-error").textContent = String(error);
  }
}

document.getElementById("selector-cancel").addEventListener("click", async () => {
  try {
    if (pendingId !== null) await invoke("cancel_pending", {id: pendingId});
    await initialize();
  } catch (error) {
    document.getElementById("selector-error").textContent = String(error);
  }
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") document.getElementById("selector-cancel").click();
  const index = Number(event.key) - 1;
  if (index >= 0) document.querySelectorAll(".selector-profile")[index]?.click();
});

initialize();
