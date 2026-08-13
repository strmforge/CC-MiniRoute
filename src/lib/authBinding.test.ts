import { describe, expect, it } from "vitest";
import {
  resolveManagedAccountId,
  resolveManagedAccountMode,
} from "./authBinding";
import type { ProviderMeta } from "@/types";

const metaWithBinding = (
  binding: NonNullable<ProviderMeta["authBinding"]>,
): ProviderMeta => ({ authBinding: binding });

describe("managed auth binding compatibility", () => {
  it("treats a legacy binding without an account as default mode", () => {
    const meta = metaWithBinding({
      source: "managed_account",
      authProvider: "codex_oauth",
    });

    expect(resolveManagedAccountMode(meta, "codex_oauth")).toBe("default");
    expect(resolveManagedAccountId(meta, "codex_oauth")).toBeNull();
  });

  it("treats a legacy binding with an accountId as fixed-account mode", () => {
    const meta = metaWithBinding({
      source: "managed_account",
      authProvider: "codex_oauth",
      accountId: "account-1",
    });

    expect(resolveManagedAccountMode(meta, "codex_oauth")).toBe("account");
    expect(resolveManagedAccountId(meta, "codex_oauth")).toBe("account-1");
  });

  it("preserves explicit pool mode", () => {
    const meta = metaWithBinding({
      source: "managed_account",
      authProvider: "codex_oauth",
      mode: "pool",
      accountId: "stale-account",
    });

    expect(resolveManagedAccountMode(meta, "codex_oauth")).toBe("pool");
    expect(resolveManagedAccountId(meta, "codex_oauth")).toBeNull();
  });

  it("ignores a stale accountId in explicit default mode", () => {
    const meta = metaWithBinding({
      source: "managed_account",
      authProvider: "codex_oauth",
      mode: "default",
      accountId: "stale-account",
    });

    expect(resolveManagedAccountMode(meta, "codex_oauth")).toBe("default");
    expect(resolveManagedAccountId(meta, "codex_oauth")).toBeNull();
  });

  it("ignores bindings owned by another auth provider", () => {
    const meta = metaWithBinding({
      source: "managed_account",
      authProvider: "github_copilot",
      mode: "account",
      accountId: "github-account",
    });

    expect(resolveManagedAccountMode(meta, "codex_oauth")).toBe("default");
    expect(resolveManagedAccountId(meta, "codex_oauth")).toBeNull();
  });
});
