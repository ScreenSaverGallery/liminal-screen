/**
 * liminal-api — IPC bridge for Liminal Screen remote options pages.
 *
 * Uses window.__TAURI__ globals (requires withGlobalTauri: true in tauri.conf.json).
 * No runtime dependency on @tauri-apps/api — safe for CDN distribution.
 *
 * Quick start:
 *   const options = await liminalAPI.getOptions();
 *   await liminalAPI.setOptions({ startsIn: 5, displayOffIn: 10, ... });
 *   await liminalAPI.startAutoSync((opts) => renderForm(opts));
 */

export type {
  AppOptions,
  MandatoryOptions,
  CustomOptions,
  SetOptionsPayload,
} from './types';
import type { AppOptions, SetOptionsPayload } from './types';

// ── Tauri globals helpers ───────────────────────────────────────────────────

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
type ListenFn = (event: string, cb: (event: { payload: unknown }) => void) => Promise<() => void>;

function tauriInvoke(): InvokeFn | null {
  if (typeof window === 'undefined') return null;
  return (window as any).__TAURI__?.core?.invoke ?? null;
}

function tauriListen(): ListenFn | null {
  if (typeof window === 'undefined') return null;
  return (window as any).__TAURI__?.event?.listen ?? null;
}

// ── Error ───────────────────────────────────────────────────────────────────

export class LiminalAPIError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message);
    this.name = 'LiminalAPIError';
    if (cause !== undefined) (this as any).cause = cause;
  }
}

// ── Mock defaults (used in non-Tauri environments) ──────────────────────────

const MOCK_OPTIONS: AppOptions = {
  saverUrl: '',
  saverUrlDebug: '',
  optionsUrl: '',
  appName: 'Liminal Screen',
  appDescription: '',
  startsIn: 5,
  displayOffIn: 10,
  requirePassIn: 0,
  runOnBattery: false,
  debug: false,
  customOptions: {},
};

// ── LiminalAPI ──────────────────────────────────────────────────────────────

export class LiminalAPI {
  private unlisteners: Array<() => void> = [];

  /** True when running inside a Liminal Screen Tauri window. */
  get isInTauri(): boolean {
    return tauriInvoke() !== null;
  }

  /** Get the full current options from the backend. */
  async getOptions(): Promise<AppOptions> {
    const invoke = tauriInvoke();
    if (!invoke) return { ...MOCK_OPTIONS };
    try {
      return (await invoke('get_options')) as AppOptions;
    } catch (e) {
      throw new LiminalAPIError('Failed to get options', e);
    }
  }

  /**
   * Persist user-controlled options to the backend.
   * Read-only identity fields (saverUrl, appName, etc.) are always preserved.
   */
  async setOptions(payload: SetOptionsPayload): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock setOptions', payload);
      return;
    }
    try {
      const current = await this.getOptions();
      await invoke('set_options', {
        options: {
          ...current,
          ...payload,
          customOptions: payload.customOptions ?? current.customOptions,
        },
      });
    } catch (e) {
      throw new LiminalAPIError('Failed to set options', e);
    }
  }

  /** Reset all options to the fork's .env defaults. Returns the new defaults. */
  async resetOptions(): Promise<AppOptions> {
    const invoke = tauriInvoke();
    if (!invoke) return { ...MOCK_OPTIONS };
    try {
      return (await invoke('factory_reset_options')) as AppOptions;
    } catch (e) {
      throw new LiminalAPIError('Failed to reset options', e);
    }
  }

  /** Trigger a screensaver preview. */
  async previewScreensaver(): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock previewScreensaver');
      return;
    }
    try {
      await invoke('preview_screensaver');
    } catch (e) {
      throw new LiminalAPIError('Failed to preview screensaver', e);
    }
  }

  /**
   * Subscribe to options-updated events dispatched via the window event bus.
   * Works without Tauri — useful when setOptions() is called locally.
   * Returns an unsubscribe function.
   */
  onOptionsUpdate(callback: (options: AppOptions) => void): () => void {
    if (typeof window === 'undefined') return () => {};
    const handler = (e: Event) => callback((e as CustomEvent<AppOptions>).detail);
    window.addEventListener('liminal:options-updated', handler);
    return () => window.removeEventListener('liminal:options-updated', handler);
  }

  /**
   * Set up auto-sync: listens for options-updated Tauri events and calls callback
   * whenever options change (e.g. user saves from another window).
   * Also re-dispatches to the window event bus so onOptionsUpdate() listeners fire.
   * Returns an unsubscribe function.
   */
  async startAutoSync(callback: (options: AppOptions) => void): Promise<() => void> {
    const listen = tauriListen();
    if (!listen) return () => {};

    const unlisten = await listen('options-updated', (event) => {
      const options = event.payload as AppOptions;
      callback(options);
      if (typeof window !== 'undefined') {
        window.dispatchEvent(
          new CustomEvent<AppOptions>('liminal:options-updated', { detail: options }),
        );
      }
    });

    this.unlisteners.push(unlisten);
    return unlisten;
  }

  /** Remove all event listeners registered via startAutoSync. */
  destroy(): void {
    for (const u of this.unlisteners) u();
    this.unlisteners = [];
  }
}

/** Shared singleton instance — use this for typical single-page setups. */
export const liminalAPI = new LiminalAPI();
