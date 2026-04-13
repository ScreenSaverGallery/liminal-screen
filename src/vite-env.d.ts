/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SAVER_URL: string;
  readonly VITE_SAVER_URL_DEBUG: string;
  readonly VITE_OPTIONS_URL: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
