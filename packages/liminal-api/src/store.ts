import { Signal } from './reactive';
import type { AppOptions, SetOptionsPayload } from './types';

/** Minimal API surface createOptionsStore depends on — satisfied by LiminalAPI. */
interface OptionsAPI {
  getOptions(): Promise<AppOptions>;
  setOptions(payload: SetOptionsPayload): Promise<void>;
  resetOptions(): Promise<AppOptions>;
  startAutoSync(callback: (options: AppOptions) => void): Promise<() => void>;
}

/**
 * Creates a reactive options store backed by the liminal-api.
 *
 * - Loads current options immediately on creation
 * - Stays in sync via startAutoSync (backend-pushed updates)
 * - Exposes save() and reset() that update the signal after each operation
 *
 * Usage:
 *   const store = createOptionsStore(liminalAPI);
 *
 *   store.signal.effect((opts) => {
 *     if (!opts) return;
 *     myInput.value = String(opts.startsIn);
 *   });
 *
 *   saveBtn.addEventListener('click', () => store.save(collectForm()));
 *   resetBtn.addEventListener('click', () => store.reset());
 *
 *   window.addEventListener('beforeunload', () => store.destroy());
 */
export function createOptionsStore(api: OptionsAPI) {
  const signal = new Signal<AppOptions | null>(null);
  let stopSync: (() => void) | null = null;

  // Initial load — async, non-blocking
  api.getOptions().then(opts => signal.set(opts)).catch(() => {});

  // Auto-sync when the backend pushes changes
  api.startAutoSync(opts => signal.set(opts)).then(stop => { stopSync = stop; }).catch(() => {});

  return {
    /** Reactive signal — subscribe with .effect() or read with .get(). */
    signal,

    /** Persist user-controlled fields to the backend, then refresh the signal. */
    async save(patch: SetOptionsPayload): Promise<void> {
      await api.setOptions(patch);
      signal.set(await api.getOptions());
    },

    /** Reset to fork defaults, then refresh the signal. */
    async reset(): Promise<void> {
      signal.set(await api.resetOptions());
    },

    /** Remove backend event listeners. Call on page unload. */
    destroy(): void {
      stopSync?.();
    },
  };
}
