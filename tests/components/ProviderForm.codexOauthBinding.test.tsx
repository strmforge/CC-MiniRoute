import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProviderForm } from "@/components/providers/forms/ProviderForm";
import { server } from "../msw/server";
import { setSettings } from "../msw/state";
import { createTestQueryClient } from "../utils/testQueryClient";

const TAURI_ENDPOINT = "http://tauri.local";
const codexAccount = {
  id: "codex-account-1",
  provider: "codex_oauth",
  login: "codex@example.com",
  avatar_url: null,
  authenticated_at: 1,
  is_default: true,
  github_domain: "github.com",
  requires_reauth: false,
  enabled: true,
  pool_enabled: true,
  status: "active",
  priority: 10,
  concurrency: 2,
};

describe("ProviderForm Codex OAuth binding", () => {
  beforeEach(() => {
    setSettings({ commonConfigConfirmed: true });
    server.use(
      http.post(`${TAURI_ENDPOINT}/auth_get_status`, async ({ request }) => {
        const { authProvider } = (await request.json()) as {
          authProvider: string;
        };
        if (authProvider === "codex_oauth") {
          return HttpResponse.json({
            provider: "codex_oauth",
            authenticated: true,
            default_account_id: codexAccount.id,
            accounts: [codexAccount],
          });
        }
        return HttpResponse.json({
          provider: authProvider,
          authenticated: false,
          default_account_id: null,
          accounts: [],
        });
      }),
    );
  });

  it.each([
    ["default", undefined],
    ["account", codexAccount.id],
    ["pool", undefined],
  ] as const)(
    "普通 Provider 的 %s 模式保存准确的账号绑定",
    async (mode, accountId) => {
      const onSubmit = vi.fn();
      const queryClient = createTestQueryClient();
      render(
        <QueryClientProvider client={queryClient}>
          <ProviderForm
            appId="claude"
            submitLabel="保存"
            onSubmit={onSubmit}
            onCancel={vi.fn()}
            initialData={{
              name: `Codex ${mode}`,
              category: "third_party",
              settingsConfig: {
                env: {
                  ANTHROPIC_BASE_URL:
                    "https://chatgpt.com/backend-api/codex",
                  ANTHROPIC_MODEL: "gpt-5.6-sol",
                },
              },
              meta: {
                providerType: "codex_oauth",
                apiFormat: "openai_responses",
                authBinding: {
                  source: "managed_account",
                  authProvider: "codex_oauth",
                  mode,
                  accountId,
                },
              },
            }}
          />
        </QueryClientProvider>,
      );

      await screen.findAllByText("codex@example.com");
      await userEvent.click(screen.getByRole("button", { name: "保存" }));

      await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
      expect(onSubmit.mock.calls[0][0].meta.authBinding).toEqual({
        source: "managed_account",
        authProvider: "codex_oauth",
        mode,
        ...(accountId ? { accountId } : {}),
      });
    },
  );

  it("固定 OpenAI Official 卡片保留明确选择的托管账号", async () => {
    const onSubmit = vi.fn();
    const queryClient = createTestQueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <ProviderForm
          appId="codex"
          providerId="codex-official"
          submitLabel="保存"
          onSubmit={onSubmit}
          onCancel={vi.fn()}
          initialData={{
            name: "OpenAI Official",
            category: "official",
            settingsConfig: { auth: {}, config: "" },
            meta: {
              authBinding: {
                source: "managed_account",
                authProvider: "codex_oauth",
                mode: "account",
                accountId: codexAccount.id,
              },
            },
          }}
        />
      </QueryClientProvider>,
    );

    await screen.findAllByText("codex@example.com");
    await userEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].meta.authBinding).toEqual({
      source: "managed_account",
      authProvider: "codex_oauth",
      mode: "account",
      accountId: codexAccount.id,
    });
  });

  it("固定 OpenAI Official 卡片切回默认模式后清除托管绑定", async () => {
    const onSubmit = vi.fn();
    const queryClient = createTestQueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <ProviderForm
          appId="codex"
          providerId="codex-official"
          submitLabel="保存"
          onSubmit={onSubmit}
          onCancel={vi.fn()}
          initialData={{
            name: "OpenAI Official",
            category: "official",
            settingsConfig: { auth: {}, config: "" },
          }}
        />
      </QueryClientProvider>,
    );

    await screen.findAllByText("codex@example.com");
    await userEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].meta.authBinding).toBeUndefined();
  });
});
