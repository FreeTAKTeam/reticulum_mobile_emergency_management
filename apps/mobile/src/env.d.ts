/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_RUNTIME_PROFILE?: "web" | "mobile";
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
