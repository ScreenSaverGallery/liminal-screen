/**
 * Reference options page — main entry point.
 * Fork developers: copy this file, add custom fields in CUSTOM_FIELDS, and host it.
 *
 * CDN usage (no npm install needed):
 *   <script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
 *
 * npm usage:
 *   import { liminalAPI, createOptionsStore } from '@liminal-screen/api';
 */

import type { AppOptions, CustomOptions, SetOptionsPayload } from '../../src/types';
import { createOptionsStore } from '../../src/store';

// Access the singleton from the global IIFE bundle (CDN) or import directly (npm)
declare const LiminalAPI: {
  liminalAPI: {
    getOptions(): Promise<AppOptions>;
    setOptions(p: SetOptionsPayload): Promise<void>;
    resetOptions(): Promise<AppOptions>;
    previewScreensaver(): Promise<void>;
    startAutoSync(cb: (o: AppOptions) => void): Promise<() => void>;
    ask(message: string, options?: Record<string, unknown>): Promise<boolean>;
    showMessage(message: string, options?: Record<string, unknown>): Promise<void>;
    isInTauri: boolean;
  };
  createOptionsStore: typeof createOptionsStore;
};

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
  // { key: 'theme',     label: 'Theme',           type: 'text',     defaultValue: 'dark', hint: 'dark | light | auto' },
  // { key: 'speed',     label: 'Animation speed', type: 'number',   defaultValue: 1.0, min: 0.1, max: 5, step: 0.1 },
  // { key: 'showClock', label: 'Show clock',       type: 'checkbox', defaultValue: true },
];

// ── UI refs ──────────────────────────────────────────────────────────────────

const $ = (id: string) => document.getElementById(id);
const startsIn     = $('starts-in')      as HTMLInputElement;
const displayOff   = $('display-off')    as HTMLInputElement;
const requirePassIn = $('require-pass-in') as HTMLInputElement;
const runOnBattery = $('run-on-battery') as HTMLInputElement;
const debugMode    = $('debug-mode')     as HTMLInputElement;
const statusDot    = $('status-dot')!;
const statusText   = $('status-text')!;

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

function collectForm(): SetOptionsPayload {
  const custom: CustomOptions = {};
  for (const def of CUSTOM_FIELDS) {
    const el = $(`custom-${def.key}`) as HTMLInputElement | null;
    if (!el) continue;
    if (def.type === 'checkbox')    custom[def.key] = el.checked;
    else if (def.type === 'number') custom[def.key] = parseFloat(el.value);
    else                            custom[def.key] = el.value;
  }
  return {
    startsIn:     parseFloat(startsIn.value),
    displayOffIn: parseFloat(displayOff.value),
    requirePassIn: parseFloat(requirePassIn.value),
    runOnBattery: runOnBattery.checked,
    debug:        debugMode.checked,
    customOptions: custom,
  };
}

function validateForm(): string | null {
  const { startsIn: s, displayOffIn: d, requirePassIn: r } = collectForm();
  if (isNaN(s)  || s < 0.1) return 'Activate After must be at least 0.1 minutes';
  if (isNaN(d)  || d < 0.5) return 'Display Off must be at least 0.5 minutes';
  if (isNaN(r)  || r < 0)   return 'Require Password must be 0 or a positive number';
  return null;
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

// ── Bootstrap ─────────────────────────────────────────────────────────────────

async function init(): Promise<void> {
  renderCustomFields();

  const api = typeof LiminalAPI !== 'undefined' ? LiminalAPI.liminalAPI : null;

  if (!api) {
    setStatus(false, 'liminal-api not loaded');
    return;
  }

  const store = createOptionsStore(api);

  // Single reactive effect — fires on load + every backend update + reset
  store.signal.effect((opts) => {
    if (!opts) return;
    startsIn.value      = String(opts.startsIn);
    displayOff.value    = String(opts.displayOffIn);
    requirePassIn.value = String(opts.requirePassIn);
    runOnBattery.checked = opts.runOnBattery;
    debugMode.checked   = opts.debug;
    setIdentity(opts);

    for (const def of CUSTOM_FIELDS) {
      const el = $(`custom-${def.key}`) as HTMLInputElement | null;
      if (!el) continue;
      const val = opts.customOptions[def.key] ?? def.defaultValue;
      if (def.type === 'checkbox') el.checked = Boolean(val);
      else el.value = String(val);
    }
  });

  setStatus(api.isInTauri, api.isInTauri ? 'Connected' : 'Preview mode (not in Tauri)');

  // ── Actions ──────────────────────────────────────────────────────────────

  async function save(): Promise<void> {
    const err = validateForm();
    if (err) { await api.showMessage(err, { title: 'Validation Error', kind: 'error' }); return; }
    try {
      await store.save(collectForm());
      setStatus(api.isInTauri, 'Saved');
      setTimeout(() => setStatus(api.isInTauri, 'Connected'), 2000);
    } catch (e) {
      await api.showMessage(`Failed to save: ${e}`, { title: 'Error', kind: 'error' });
    }
  }

  async function reset(): Promise<void> {
    if (!await api.ask('Reset all options to defaults?', { title: 'Reset', kind: 'warning', okLabel: 'Reset', cancelLabel: 'Cancel' })) return;
    try {
      await store.reset();
      setStatus(api.isInTauri, 'Reset to defaults');
      setTimeout(() => setStatus(api.isInTauri, 'Connected'), 2000);
    } catch (e) {
      await api.showMessage(`Failed to reset: ${e}`, { title: 'Error', kind: 'error' });
    }
  }

  $('save-btn')?.addEventListener('click',    save);
  $('reset-btn')?.addEventListener('click',   reset);
  $('preview-btn')?.addEventListener('click', () => api.previewScreensaver());

  // Auto-save on any field change
  document.querySelectorAll<HTMLInputElement>('input').forEach((input) => {
    input.addEventListener('change', save);
  });

  window.addEventListener('beforeunload', () => store.destroy());
}

document.addEventListener('DOMContentLoaded', init);
