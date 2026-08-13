import { invoke } from "@tauri-apps/api/core";

export type ManagedAuthProvider =
  | "github_copilot"
  | "codex_oauth"
  | "xai_oauth";

export interface ManagedAuthAccount {
  id: string;
  provider: ManagedAuthProvider;
  login: string;
  avatar_url: string | null;
  authenticated_at: number;
  is_default: boolean;
  github_domain: string;
  requires_reauth: boolean;
  enabled?: boolean;
  pool_enabled?: boolean;
  status?: "active" | "cooldown" | "disabled" | "expired" | "reauth_required";
  last_error?: string | null;
  cooldown_until_ms?: number | null;
  priority?: number;
  concurrency?: number;
  expires_at_ms?: number | null;
  auto_pause_on_expired?: boolean;
  renewable?: boolean;
  plan_type?: string | null;
  proxy_url?: string | null;
}

export interface ManagedAuthStatus {
  provider: ManagedAuthProvider;
  authenticated: boolean;
  default_account_id: string | null;
  migration_error?: string | null;
  accounts: ManagedAuthAccount[];
}

export interface ManagedAuthDeviceCodeResponse {
  provider: ManagedAuthProvider;
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export interface CodexOAuthImportOptions {
  updateExisting?: boolean;
  priority?: number;
  concurrency?: number;
  poolEnabled?: boolean;
  expiresAtMs?: number | null;
  autoPauseOnExpired?: boolean;
  proxyUrl?: string | null;
}

export interface CodexOAuthImportItem {
  index: number;
  name: string | null;
  action: "created" | "updated" | "skipped" | "failed";
  accountId: string | null;
  message: string | null;
}

export interface CodexOAuthImportResult {
  total: number;
  created: number;
  updated: number;
  skipped: number;
  failed: number;
  items: CodexOAuthImportItem[];
  warnings: string[];
  errors: string[];
}

export interface CodexOAuthAccountUpdate {
  enabled?: boolean;
  poolEnabled?: boolean;
  priority?: number;
  concurrency?: number;
  expiresAtMs?: number;
  clearExpiresAt?: boolean;
  autoPauseOnExpired?: boolean;
  proxyUrl?: string;
  clearProxyUrl?: boolean;
}

export async function authStartLogin(
  authProvider: ManagedAuthProvider,
  githubDomain?: string,
): Promise<ManagedAuthDeviceCodeResponse> {
  return invoke<ManagedAuthDeviceCodeResponse>("auth_start_login", {
    authProvider,
    githubDomain: githubDomain || null,
  });
}

export async function authPollForAccount(
  authProvider: ManagedAuthProvider,
  deviceCode: string,
  githubDomain?: string,
): Promise<ManagedAuthAccount | null> {
  return invoke<ManagedAuthAccount | null>("auth_poll_for_account", {
    authProvider,
    deviceCode,
    githubDomain: githubDomain || null,
  });
}

export async function authListAccounts(
  authProvider: ManagedAuthProvider,
): Promise<ManagedAuthAccount[]> {
  return invoke<ManagedAuthAccount[]>("auth_list_accounts", {
    authProvider,
  });
}

export async function authGetStatus(
  authProvider: ManagedAuthProvider,
): Promise<ManagedAuthStatus> {
  return invoke<ManagedAuthStatus>("auth_get_status", {
    authProvider,
  });
}

export async function authRemoveAccount(
  authProvider: ManagedAuthProvider,
  accountId: string,
): Promise<void> {
  return invoke("auth_remove_account", {
    authProvider,
    accountId,
  });
}

export async function authSetDefaultAccount(
  authProvider: ManagedAuthProvider,
  accountId: string,
): Promise<void> {
  return invoke("auth_set_default_account", {
    authProvider,
    accountId,
  });
}

export async function importCodexOauthAccounts(
  content: string,
  options: CodexOAuthImportOptions = {},
): Promise<CodexOAuthImportResult> {
  return invoke<CodexOAuthImportResult>("import_codex_oauth_accounts", {
    content,
    options,
  });
}

export async function updateCodexOauthAccount(
  accountId: string,
  update: CodexOAuthAccountUpdate,
): Promise<void> {
  return invoke("update_codex_oauth_account", {
    accountId,
    update,
  });
}

export async function authLogout(
  authProvider: ManagedAuthProvider,
): Promise<void> {
  return invoke("auth_logout", {
    authProvider,
  });
}

export const authApi = {
  authStartLogin,
  authPollForAccount,
  authListAccounts,
  authGetStatus,
  authRemoveAccount,
  authSetDefaultAccount,
  importCodexOauthAccounts,
  updateCodexOauthAccount,
  authLogout,
};
