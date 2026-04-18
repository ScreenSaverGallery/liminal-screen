// main.ts
var CUSTOM_FIELDS = [];
var $ = (id) => document.getElementById(id);
var startsIn = $("starts-in");
var displayOff = $("display-off");
var requirePassIn = $("require-pass-in");
var runOnBattery = $("run-on-battery");
var debugMode = $("debug-mode");
var statusDot = $("status-dot");
var statusText = $("status-text");
var current = null;
var dirty = false;
var api = typeof LiminalAPI !== "undefined" ? LiminalAPI.liminalAPI : null;
async function init() {
  renderCustomFields();
  setupListeners();
  if (!api) {
    setStatus(false, "liminal-api not loaded");
    return;
  }
  try {
    current = await api.getOptions();
    populateForm(current);
    setIdentity(current);
    setStatus(api.isInTauri, api.isInTauri ? "Connected" : "Preview mode (not in Tauri)");
    await api.startAutoSync((opts) => {
      current = opts;
      populateForm(opts);
    });
  } catch (e) {
    setStatus(false, `Error: ${e}`);
  }
}
function renderCustomFields() {
  const container = $("custom-fields");
  if (CUSTOM_FIELDS.length === 0)
    return;
  const section = document.createElement("div");
  section.className = "section";
  section.innerHTML = "<h2>Custom</h2>";
  for (const def of CUSTOM_FIELDS) {
    if (def.type === "checkbox") {
      const wrap = document.createElement("div");
      wrap.className = "toggle-field";
      wrap.innerHTML = `
        <input type="checkbox" id="custom-${def.key}" />
        <label for="custom-${def.key}">${def.label}</label>
      `;
      section.appendChild(wrap);
    } else {
      const wrap = document.createElement("div");
      wrap.className = "field";
      const attrs = def.type === "number" ? `min="${def.min ?? ""}" max="${def.max ?? ""}" step="${def.step ?? 1}"` : "";
      wrap.innerHTML = `
        <label for="custom-${def.key}">${def.label}</label>
        <input type="${def.type}" id="custom-${def.key}" ${attrs} value="${def.defaultValue}" />
        ${def.hint ? `<div class="hint">${def.hint}</div>` : ""}
      `;
      section.appendChild(wrap);
    }
  }
  container.appendChild(section);
}
function populateForm(opts) {
  startsIn.value = String(opts.startsIn);
  displayOff.value = String(opts.displayOffIn);
  requirePassIn.value = String(opts.requirePassIn);
  runOnBattery.checked = opts.runOnBattery;
  debugMode.checked = opts.debug;
  for (const def of CUSTOM_FIELDS) {
    const el = $(`custom-${def.key}`);
    if (!el)
      continue;
    const val = opts.customOptions[def.key] ?? def.defaultValue;
    if (def.type === "checkbox") {
      el.checked = Boolean(val);
    } else {
      el.value = String(val);
    }
  }
  dirty = false;
}
function collectForm() {
  const mandatory = {
    startsIn: parseFloat(startsIn.value),
    displayOffIn: parseFloat(displayOff.value),
    requirePassIn: parseFloat(requirePassIn.value),
    runOnBattery: runOnBattery.checked,
    debug: debugMode.checked
  };
  const custom = {};
  for (const def of CUSTOM_FIELDS) {
    const el = $(`custom-${def.key}`);
    if (!el)
      continue;
    if (def.type === "checkbox") {
      custom[def.key] = el.checked;
    } else if (def.type === "number") {
      custom[def.key] = parseFloat(el.value);
    } else {
      custom[def.key] = el.value;
    }
  }
  return { mandatory, custom };
}
function setIdentity(opts) {
  const nameEl = $("app-name");
  const descEl = $("app-description");
  if (nameEl && opts.appName)
    nameEl.textContent = `${opts.appName} — Options`;
  if (descEl)
    descEl.textContent = opts.appDescription ?? "";
  document.title = opts.appName ? `${opts.appName} Options` : "Options";
}
function setStatus(connected, text) {
  statusDot.className = `dot${connected ? " connected" : ""}`;
  statusText.textContent = text;
}
async function save() {
  if (!api)
    return;
  const { mandatory, custom } = collectForm();
  if (isNaN(mandatory.startsIn) || mandatory.startsIn < 0.1) {
    alert("Activate After must be at least 0.1 minutes");
    return;
  }
  if (isNaN(mandatory.displayOffIn) || mandatory.displayOffIn < 0.5) {
    alert("Display Off must be at least 0.5 minutes");
    return;
  }
  if (isNaN(mandatory.requirePassIn) || mandatory.requirePassIn < 0) {
    alert("Require Password must be 0 or a positive number");
    return;
  }
  try {
    await api.setOptions({ ...mandatory, customOptions: custom });
    dirty = false;
    setStatus(api.isInTauri, "Saved");
    setTimeout(() => setStatus(api.isInTauri, "Connected"), 2000);
  } catch (e) {
    alert(`Failed to save: ${e}`);
  }
}
async function reset() {
  if (!api)
    return;
  if (!confirm("Reset all options to defaults?"))
    return;
  try {
    const defaults = await api.resetOptions();
    current = defaults;
    populateForm(defaults);
    setStatus(api.isInTauri, "Reset to defaults");
    setTimeout(() => setStatus(api.isInTauri, "Connected"), 2000);
  } catch (e) {
    alert(`Failed to reset: ${e}`);
  }
}
function setupListeners() {
  $("save-btn")?.addEventListener("click", save);
  $("preview-btn")?.addEventListener("click", () => api?.previewScreensaver());
  $("reset-btn")?.addEventListener("click", reset);
  const inputs = document.querySelectorAll("input");
  inputs.forEach((input) => {
    input.addEventListener("change", () => {
      dirty = true;
      save();
    });
  });
  window.addEventListener("beforeunload", (e) => {
    if (dirty) {
      e.preventDefault();
      save();
    }
  });
}
document.addEventListener("DOMContentLoaded", init);
