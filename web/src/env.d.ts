/// <reference types="astro/client" />

interface ImportMetaEnv {
  readonly PUBLIC_GATE_ACCOUNT?: string;
  readonly PUBLIC_NEAR_RPC?: string;
  readonly PUBLIC_VERIFIER_URL?: string;
  readonly PUBLIC_BASE_RPC?: string;
  readonly PUBLIC_REPO_URL?: string;
  readonly PUBLIC_X_POST_URL?: string;
  readonly PUBLIC_DEMO_ACCOUNT_ID?: string;
  readonly PUBLIC_DEMO_SECRET_KEY?: string;
  readonly PUBLIC_ATTACK_IMPL?: string;
  readonly PUBLIC_ATTACK_CODEHASH?: string;
  readonly PUBLIC_ATTACK_CURRENT_IMPL?: string;
  readonly PUBLIC_ATTACK_INIT_CALLDATA?: string;
  readonly PUBLIC_HONEST_IMPL?: string;
  readonly PUBLIC_HONEST_CODEHASH?: string;
  readonly PUBLIC_HONEST_CURRENT_IMPL?: string;
  readonly PUBLIC_HONEST_INIT_CALLDATA?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
