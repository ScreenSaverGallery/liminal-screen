/**
 * CDN / IIFE entry point.
 *
 * Bun's iife output doesn't expose module exports on its own, so this entry
 * attaches the public surface to `globalThis.LiminalAPI`:
 *
 *   <script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
 *   <script>
 *     const { liminalAPI, createOptionsStore } = LiminalAPI;
 *   </script>
 *
 * The npm/ESM entry point is `src/index.ts` — import from there instead.
 */
import {
  liminalAPI,
  LiminalAPI,
  LiminalAPIError,
  Signal,
  createOptionsStore,
} from './index';

const surface = {
  liminalAPI,
  LiminalAPI,
  LiminalAPIError,
  Signal,
  createOptionsStore,
};

export type LiminalAPIGlobal = typeof surface;

declare global {
  // eslint-disable-next-line no-var
  var LiminalAPI: LiminalAPIGlobal;
}

(globalThis as unknown as { LiminalAPI: LiminalAPIGlobal }).LiminalAPI = surface;

export {};
