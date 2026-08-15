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
 *
 * Reactive quick start:
 *   const store = createOptionsStore(liminalAPI);
 *   store.signal.effect((opts) => { if (opts) myInput.value = String(opts.startsIn); });
 *   saveBtn.addEventListener('click', () => store.save(collectForm()));
 */

export type {
  AppOptions,
  MandatoryOptions,
  CustomOptions,
  SetOptionsPayload,
  UpdateInfo,
  OsScreensaverStatus,
} from './types';
import type {
  AppOptions,
  SetOptionsPayload,
  UpdateInfo,
  OsScreensaverStatus,
} from './types';

export { Signal } from './reactive';
export { createOptionsStore } from './store';

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

function tauriDialog(): { ask(message: string, options?: Record<string, unknown>): Promise<boolean>; message(message: string, options?: Record<string, unknown>): Promise<void> } | null {
  if (typeof window === 'undefined') return null;
  const dialog = (window as any).__TAURI__?.dialog;
  if (!dialog?.ask || !dialog?.message) return null;
  return { ask: dialog.ask.bind(dialog), message: dialog.message.bind(dialog) };
}

// ── Error ───────────────────────────────────────────────────────────────────

export class LiminalAPIError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message);
    this.name = 'LiminalAPIError';
    if (cause !== undefined) (this as any).cause = cause;
  }
}

// ── Constants ───────────────────────────────────────────────────────────────

/** Webview window label used for the preview opened from remote options. */
const PREVIEW_LABEL = 'liminal-preview';

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
  instanceId: '',
  notificationUrl: '',
  notificationCheckIntervalSecs: 3600,
  notificationsEnabled: false,
  autostart: false,
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

  /**
   * Show a confirmation dialog (Yes/No). Returns true if the user confirms.
   * Falls back to window.confirm() in non-Tauri environments.
   */
  async ask(message: string, options?: Record<string, unknown>): Promise<boolean> {
    const dialog = tauriDialog();
    if (dialog) {
      try {
        return await dialog.ask(message, options);
      } catch (e) {
        console.warn('[liminal-api] dialog plugin failed, falling back to confirm()', e);
      }
    }
    if (typeof window === 'undefined') return false;
    return window.confirm(message);
  }

  /**
   * Open an external URL in the user's default browser/application via the
   * Tauri `opener` plugin. Falls back to `window.open(url, '_blank')` outside
   * Tauri or if the opener call fails.
   *
   * Requires the `opener:default` (or `opener:allow-open-url`) permission in the
   * window's capability. The opener scope permits `http:`, `https:`, `mailto:`
   * and `tel:` by default.
   *
   * @param url      The URL to open.
   * @param openWith Optional application name to open the URL with.
   */
  async openUrl(url: string, openWith?: string): Promise<void> {
    const invoke = tauriInvoke();
    if (invoke) {
      try {
        await invoke('plugin:opener|open_url', { url, with: openWith });
        return;
      } catch (e) {
        console.warn('[liminal-api] opener plugin failed, falling back to window.open', e);
      }
    }
    if (typeof window !== 'undefined') {
      window.open(url, '_blank', 'noopener');
    }
  }

  /**
   * Show a message dialog. Falls back to window.alert() in non-Tauri environments.
   */
  async showMessage(message: string, options?: Record<string, unknown>): Promise<void> {
    const dialog = tauriDialog();
    if (dialog) {
      try {
        await dialog.message(message, options);
        return;
      } catch (e) {
        console.warn('[liminal-api] dialog plugin failed, falling back to alert()', e);
      }
    }
    if (typeof window === 'undefined') return;
    window.alert(message);
  }

  /**
   * Open a screensaver preview window. Reads the current saver URL from the
   * backend (honoring the debug flag) and creates a dedicated preview webview
   * directly via the `create_preview_window` command — no dependency on the
   * main window's event relay, so it works reliably from remote options pages.
   * No-op outside Tauri.
   */
  async previewScreensaver(): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock previewScreensaver');
      return;
    }
    try {
      const opts = await this.getOptions();
      const url = opts.debug ? opts.saverUrlDebug : opts.saverUrl;
      if (!url) {
        throw new LiminalAPIError('No saver URL configured for preview');
      }
      await invoke('create_preview_window', { url, label: PREVIEW_LABEL });
    } catch (e) {
      if (e instanceof LiminalAPIError) throw e;
      throw new LiminalAPIError('Failed to preview screensaver', e);
    }
  }

  /**
   * Close the options window this page is running in — for a "Close" or "Done"
   * button on the options page.
   *
   * Uses the backend's `close_options` command rather than the Tauri window API,
   * which a remote page can't reach without being granted window permissions on
   * its own origin. A no-op if the window is already gone, and outside Tauri.
   *
   * Note this closes the *window*, not just the page: any unsaved form state goes
   * with it, so call `setOptions()` (or `store.save()`) first if that matters.
   */
  async closeOptions(): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock closeOptions');
      return;
    }
    try {
      await invoke('close_options');
    } catch (e) {
      throw new LiminalAPIError('Failed to close the options window', e);
    }
  }

  /**
   * The running application version. Reads the injected navigator.liminalScreen
   * snapshot when present (zero IPC), else asks the backend. Empty outside Tauri.
   */
  async getVersion(): Promise<string> {
    const injected =
      typeof window === 'undefined'
        ? undefined
        : (window as any).navigator?.liminalScreen?.version;
    if (typeof injected === 'string' && injected) return injected;
    const invoke = tauriInvoke();
    if (!invoke) return '';
    try {
      return (await invoke('get_app_version')) as string;
    } catch (e) {
      throw new LiminalAPIError('Failed to get app version', e);
    }
  }

  /**
   * Read the OS-native screensaver configuration to detect conflicts with
   * Liminal. In non-Tauri environments reports "not detected".
   */
  async getOsScreensaverStatus(): Promise<OsScreensaverStatus> {
    const invoke = tauriInvoke();
    if (!invoke) return { detected: false, enabled: false, idleSeconds: null };
    try {
      return (await invoke('get_os_screensaver_status')) as OsScreensaverStatus;
    } catch (e) {
      throw new LiminalAPIError('Failed to get OS screensaver status', e);
    }
  }

  /**
   * Disable the OS-native screensaver so it can't appear over Liminal. The
   * prior timeout is saved so it can be restored (see restoreOsScreensaver).
   */
  async disableOsScreensaver(): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock disableOsScreensaver');
      return;
    }
    try {
      await invoke('disable_os_screensaver');
    } catch (e) {
      throw new LiminalAPIError('Failed to disable OS screensaver', e);
    }
  }

  /** Restore the OS-native screensaver to the timeout saved when it was disabled. */
  async restoreOsScreensaver(): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock restoreOsScreensaver');
      return;
    }
    try {
      await invoke('restore_os_screensaver');
    } catch (e) {
      throw new LiminalAPIError('Failed to restore OS screensaver', e);
    }
  }

  /**
   * The OS screensaver timeout (seconds) Liminal saved when it disabled the
   * screensaver, or null if Liminal hasn't disabled it. Drives the "Restore"
   * affordance on the options page.
   */
  async getSavedOsScreensaverIdle(): Promise<number | null> {
    const invoke = tauriInvoke();
    if (!invoke) return null;
    try {
      return ((await invoke('get_saved_os_screensaver_idle')) as number | null) ?? null;
    } catch (e) {
      throw new LiminalAPIError('Failed to get saved OS screensaver setting', e);
    }
  }

  /**
   * True when another process — a video player, video call, etc. — is
   * holding a display-sleep-blocking power assertion. The screensaver engine
   * already treats this as user activity and won't activate while it's true;
   * expose it so an options page can tell the user *why* the saver hasn't
   * started instead of leaving them thinking it's broken.
   *
   * macOS only for now. Always `false` on Windows/Linux and outside Tauri.
   */
  async isMediaActive(): Promise<boolean> {
    const invoke = tauriInvoke();
    if (!invoke) return false;
    try {
      return (await invoke('is_media_active')) as boolean;
    } catch (e) {
      throw new LiminalAPIError('Failed to check media-active status', e);
    }
  }

  /**
   * Name of the process holding the display-sleep assertion `isMediaActive()`
   * detected (e.g. `"LocalSend"`), or `null` if nothing is. Pair with
   * `isMediaActive()` to explain a suppressed saver:
   *
   * ```javascript
   * if (await liminalAPI.isMediaActive()) {
   *   const who = await liminalAPI.getMediaBlockerName();
   *   message = who ? `${who} is blocking ${appName} from starting.` : 'Something is blocking the screensaver from starting.';
   * }
   * ```
   *
   * Shells out to read the OS's per-process assertion list, so prefer calling
   * this only when `isMediaActive()` is already `true` rather than on every
   * poll tick. macOS only for now. Always `null` on Windows/Linux and outside
   * Tauri.
   */
  async getMediaBlockerName(): Promise<string | null> {
    const invoke = tauriInvoke();
    if (!invoke) return null;
    try {
      return ((await invoke('get_media_blocker_name')) as string | null) ?? null;
    } catch (e) {
      throw new LiminalAPIError('Failed to get media blocker name', e);
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
   *
   * Never throws: if the event listener can't be registered — e.g. the options
   * window's capability doesn't grant `core:event:allow-listen` to the page's
   * remote origin — the failure is logged and a no-op unsubscribe is returned, so
   * the page still works without live updates.
   */
  async startAutoSync(callback: (options: AppOptions) => void): Promise<() => void> {
    const listen = tauriListen();
    if (!listen) return () => {};

    let unlisten: () => void;
    try {
      unlisten = await listen('options-updated', (event) => {
        const options = event.payload as AppOptions;
        callback(options);
        if (typeof window !== 'undefined') {
          window.dispatchEvent(
            new CustomEvent<AppOptions>('liminal:options-updated', { detail: options }),
          );
        }
      });
    } catch (e) {
      console.warn('[liminal-api] could not subscribe to options-updated events', e);
      return () => {};
    }

    this.unlisteners.push(unlisten);
    return unlisten;
  }

  /**
   * Check for an application update. Returns the update info when one is
   * available, null otherwise (or when running outside Tauri).
   * Also causes the backend to emit `update-available` / `update-not-available`.
   */
  async checkForUpdates(): Promise<UpdateInfo | null> {
    const invoke = tauriInvoke();
    if (!invoke) return null;
    try {
      return ((await invoke('check_for_updates')) as UpdateInfo | null) ?? null;
    } catch (e) {
      throw new LiminalAPIError('Failed to check for updates', e);
    }
  }

  /**
   * Download and install a pending update. The backend emits
   * `update-download-progress` events while downloading, then restarts the app.
   */
  async installUpdate(): Promise<void> {
    const invoke = tauriInvoke();
    if (!invoke) {
      console.log('[liminal-api] mock installUpdate');
      return;
    }
    try {
      await invoke('install_update');
    } catch (e) {
      throw new LiminalAPIError('Failed to install update', e);
    }
  }

  /**
   * Subscribe to `update-available` events (fired by both the startup check
   * and manual checks). Returns an unsubscribe function. No-op outside Tauri, or
   * if the subscription is rejected by the window's capability.
   */
  onUpdateAvailable(callback: (info: UpdateInfo) => void): () => void {
    const listen = tauriListen();
    if (!listen) return () => {};
    const unlistenPromise = listen('update-available', (event) => {
      callback(event.payload as UpdateInfo);
    }).catch((e) => {
      console.warn('[liminal-api] could not subscribe to update-available events', e);
      return () => {};
    });
    unlistenPromise.then((unlisten) => this.unlisteners.push(unlisten));
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }

  /** Remove all event listeners registered via startAutoSync/onUpdateAvailable. */
  destroy(): void {
    for (const u of this.unlisteners) u();
    this.unlisteners = [];
  }
}

/** Shared singleton instance — use this for typical single-page setups. */
export const liminalAPI = new LiminalAPI();
