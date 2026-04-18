/**
 * Reference options page — main entry point.
 * Fork developers: copy this file, add custom fields in CUSTOM_FIELDS, and host it.
 *
 * CDN usage (no npm install needed):
 *   <script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
 *
 * npm usage:
 *   import { liminalAPI } from '@liminal-screen/api';
 */

import type { AppOptions, CustomOptions } from '../packages/liminal-api/src/types';

// Access the singleton from the global IIFE bundle (CDN) or import directly (npm)
// When loaded via the global bundle, LiminalAPI namespace is on window.
declare const LiminalAPI: { liminalAPI: { getOptions(): Promise<AppOptions>; setOptions(p: any): Promise<void>; resetOptions(): Promise<AppOptions>; previewScreensaver(): Promise<void>; startAutoSync(cb: (o: AppOptions) => void): Promise<() => void>; isInTauri: boolean; } };

// ── Fork customization ───────────────────────────────────────────────────────
//
// Add your own fields here. Each entry defines a custom option that will be
// appended to the screensaver URL as a query parameter.
//
// Supported types: 'text' | 'number' | 'checkbox'
//
interface CustomFieldDef {
  key: string;
  label: string;
  hint?: string;
  type: 'text' | 'number' | 'checkbox';
  defaultValue: string | number | boolean;
  min?: number;
  max?: number;
  step?: number;
}

const CUSTOM_FIELDS: CustomFieldDef[] = [
  // Example — uncomment and edit to add your own fields:
  // { key: 'theme', label: 'Theme', type: 'text', defaultValue: 'dark', hint: 'dark | light | auto' },
  // { key: 'speed', label: 'Animation speed', type: 'number', defaultValue: 1.0, min: 0.1, max: 5, step: 0.1 },
  // { key: 'showClock', label: 'Show clock', type: 'checkbox', defaultValue: true },
];

// ── UI refs ──────────────────────────────────────────────────────────────────

const $ = (id: string) => document.getElementById(id);
const startsIn = $('starts-in') as HTMLInputElement;
const displayOff = $('display-off') as HTMLInputElement;
const requirePassIn = $('require-pass-in') as HTMLInputElement;
const runOnBattery = $('run-on-battery') as HTMLInputElement;
const debugMode = $('debug-mode') as HTMLInputElement;
const statusDot = $('status-dot')!;
const statusText = $('status-text')!;

// ── State ────────────────────────────────────────────────────────────────────

let current: AppOptions | null = null;
let dirty = false;

const api = typeof LiminalAPI !== 'undefined' ? LiminalAPI.liminalAPI : null;

// ── Bootstrap ────────────────────────────────────────────────────────────────

async function init(): Promise<void> {
  renderCustomFields();
  setupListeners();

  if (!api) {
    setStatus(false, 'liminal-api not loaded');
    return;
  }

  try {
    current = await api.getOptions();
    populateForm(current);
    setIdentity(current);
    setStatus(api.isInTauri, api.isInTauri ? 'Connected' : 'Preview mode (not in Tauri)');

    await api.startAutoSync((opts) => {
      current = opts;
      populateForm(opts);
    });
  } catch (e) {
    setStatus(false, `Error: ${e}`);
  }
}

// ── Custom fields ─────────────────────────────────────────────────────────────

function renderCustomFields(): void {
  const container = $('custom-fields')!;
  if (CUSTOM_FIELDS.length === 0) return;

  const section = document.createElement('div');
  section.className = 'section';
  section.innerHTML = '<h2>Custom</h2>';

  for (const def of CUSTOM_FIELDS) {
    if (def.type === 'checkbox') {
      const wrap = document.createElement('div');
      wrap.className = 'toggle-field';
      wrap.innerHTML = `
        <input type="checkbox" id="custom-${def.key}" />
        <label for="custom-${def.key}">${def.label}</label>
      `;
      section.appendChild(wrap);
    } else {
      const wrap = document.createElement('div');
      wrap.className = 'field';
      const attrs = def.type === 'number'
        ? `min="${def.min ?? ''}" max="${def.max ?? ''}" step="${def.step ?? 1}"`
        : '';
      wrap.innerHTML = `
        <label for="custom-${def.key}">${def.label}</label>
        <input type="${def.type}" id="custom-${def.key}" ${attrs} value="${def.defaultValue}" />
        ${def.hint ? `<div class="hint">${def.hint}</div>` : ''}
      `;
      section.appendChild(wrap);
    }
  }

  container.appendChild(section);
}

// ── Form ──────────────────────────────────────────────────────────────────────

function populateForm(opts: AppOptions): void {
  startsIn.value = String(opts.startsIn);
  displayOff.value = String(opts.displayOffIn);
  requirePassIn.value = String(opts.requirePassIn);
  runOnBattery.checked = opts.runOnBattery;
  debugMode.checked = opts.debug;

  for (const def of CUSTOM_FIELDS) {
    const el = $(`custom-${def.key}`) as HTMLInputElement | null;
    if (!el) continue;
    const val = opts.customOptions[def.key] ?? def.defaultValue;
    if (def.type === 'checkbox') {
      el.checked = Boolean(val);
    } else {
      el.value = String(val);
    }
  }

  dirty = false;
}

function collectForm(): { mandatory: any; custom: CustomOptions } {
  const mandatory = {
    startsIn: parseFloat(startsIn.value),
    displayOffIn: parseFloat(displayOff.value),
    requirePassIn: parseFloat(requirePassIn.value),
    runOnBattery: runOnBattery.checked,
    debug: debugMode.checked,
  };

  const custom: CustomOptions = {};
  for (const def of CUSTOM_FIELDS) {
    const el = $(`custom-${def.key}`) as HTMLInputElement | null;
    if (!el) continue;
    if (def.type === 'checkbox') {
      custom[def.key] = el.checked;
    } else if (def.type === 'number') {
      custom[def.key] = parseFloat(el.value);
    } else {
      custom[def.key] = el.value;
    }
  }

  return { mandatory, custom };
}

function setIdentity(opts: AppOptions): void {
  const nameEl = $('app-name');
  const descEl = $('app-description');
  if (nameEl && opts.appName) nameEl.textContent = `${opts.appName} — Options`;
  if (descEl) descEl.textContent = opts.appDescription ?? '';
  document.title = opts.appName ? `${opts.appName} Options` : 'Options';
}

function setStatus(connected: boolean, text: string): void {
  statusDot.className = `dot${connected ? ' connected' : ''}`;
  statusText.textContent = text;
}

// ── Actions ───────────────────────────────────────────────────────────────────

async function save(): Promise<void> {
  if (!api) return;
  const { mandatory, custom } = collectForm();

  if (isNaN(mandatory.startsIn) || mandatory.startsIn < 0.1) {
    alert('Activate After must be at least 0.1 minutes');
    return;
  }
  if (isNaN(mandatory.displayOffIn) || mandatory.displayOffIn < 0.5) {
    alert('Display Off must be at least 0.5 minutes');
    return;
  }
  if (isNaN(mandatory.requirePassIn) || mandatory.requirePassIn < 0) {
    alert('Require Password must be 0 or a positive number');
    return;
  }

  try {
    await api.setOptions({ ...mandatory, customOptions: custom });
    dirty = false;
    setStatus(api.isInTauri, 'Saved');
    setTimeout(() => setStatus(api.isInTauri, 'Connected'), 2000);
  } catch (e) {
    alert(`Failed to save: ${e}`);
  }
}

async function reset(): Promise<void> {
  if (!api) return;
  if (!confirm('Reset all options to defaults?')) return;
  try {
    const defaults = await api.resetOptions();
    current = defaults;
    populateForm(defaults);
    setStatus(api.isInTauri, 'Reset to defaults');
    setTimeout(() => setStatus(api.isInTauri, 'Connected'), 2000);
  } catch (e) {
    alert(`Failed to reset: ${e}`);
  }
}

// ── Event listeners ───────────────────────────────────────────────────────────

function setupListeners(): void {
  $('save-btn')?.addEventListener('click', save);
  $('preview-btn')?.addEventListener('click', () => api?.previewScreensaver());
  $('reset-btn')?.addEventListener('click', reset);

  // Save on change (auto-save)
  const inputs = document.querySelectorAll<HTMLInputElement>('input');
  inputs.forEach((input) => {
    input.addEventListener('change', () => {
      dirty = true;
      save();
    });
  });

  // Save unsaved changes before the window closes
  window.addEventListener('beforeunload', (e) => {
    if (dirty) {
      e.preventDefault();
      save();
    }
  });
}

// ── Start ─────────────────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', init);
