import type { ProviderMeta } from "@/types";

export function resolveManagedAccountId(
  meta: ProviderMeta | undefined,
  authProvider: string,
): string | null {
  const binding = meta?.authBinding;

  if (
    binding?.source === "managed_account" &&
    binding.authProvider === authProvider &&
    (binding.mode === undefined || binding.mode === "account")
  ) {
    return binding.accountId ?? null;
  }

  if (authProvider === "github_copilot") {
    return meta?.githubAccountId ?? null;
  }

  return null;
}

export function resolveManagedAccountMode(
  meta: ProviderMeta | undefined,
  authProvider: string,
): "default" | "account" | "pool" {
  const binding = meta?.authBinding;
  if (
    binding?.source !== "managed_account" ||
    binding.authProvider !== authProvider
  ) {
    return "default";
  }
  return binding.mode ?? (binding.accountId ? "account" : "default");
}
