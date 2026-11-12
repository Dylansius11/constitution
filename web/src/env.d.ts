/// <reference types="astro/client" />

interface ImportMetaEnv {
  readonly PUBLIC_GATE_ACCOUNT?: string;
  readonly PUBLIC_NEAR_RPC?: string;
  readonly PUBLIC_BASE_RPC?: string;
  readonly PUBLIC_REPO_URL?: string;
  readonly PUBLIC_X_POST_URL?: string;
  readonly PUBLIC_DEMO_GATE_ACCOUNT?: string;
  readonly PUBLIC_DEMO_VERIFIER_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
