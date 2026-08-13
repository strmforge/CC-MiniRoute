import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ClaudeDesktopProviderForm } from "@/components/providers/forms/ClaudeDesktopProviderForm";
import { createTestQueryClient } from "../utils/testQueryClient";
import { server } from "../msw/server";

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

vi.mock("@/lib/api/providers", () => ({
  providersApi: {
    getClaudeDesktopDefaultRoutes: () => Promise.resolve([]),
  },
}));

function renderForm(
  initialData: ComponentProps<typeof ClaudeDesktopProviderForm>["initialData"],
  onSubmit = vi.fn(),
) {
  const queryClient = createTestQueryClient();
  const view = render(
    <QueryClientProvider client={queryClient}>
      <ClaudeDesktopProviderForm
        submitLabel="保存"
        onSubmit={onSubmit}
        onCancel={vi.fn()}
        initialData={initialData}
      />
    </QueryClientProvider>,
  );
  return { ...view, onSubmit };
}

describe("ClaudeDesktopProviderForm", () => {
  beforeEach(() => {
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

  it.each(["github_copilot", "codex_oauth", "xai_oauth"])(
    "托管 OAuth %s 即使旧数据是 direct 也强制开启模型映射",
    (providerType) => {
      renderForm({
        name: "Managed OAuth Provider",
        category: "third_party",
        settingsConfig: {
          env: {
            ANTHROPIC_BASE_URL: "https://api.example.com",
          },
        },
        meta: {
          providerType,
          claudeDesktopMode: "direct",
          apiFormat: "anthropic",
          claudeDesktopModelRoutes: {
            "claude-sonnet-5": { model: "upstream-model" },
          },
        },
      });

      const modelMappingToggle = screen.getByRole("switch", {
        name: "需要模型映射",
      });
      expect(modelMappingToggle).toBeChecked();
      expect(modelMappingToggle).toBeDisabled();
    },
  );

  it("直连预设保留自己的模型列表", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    renderForm(undefined, onSubmit);

    await user.click(screen.getByRole("button", { name: /PackyCode/ }));
    await user.type(screen.getByLabelText("API Key"), "sk-test");
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalled());
    expect(
      onSubmit.mock.calls[0][0].meta.claudeDesktopModelRoutes,
    ).toMatchObject({
      "claude-sonnet-5": { model: "claude-sonnet-5" },
      "claude-opus-5": { model: "claude-opus-5" },
      "claude-haiku-4-5": { model: "claude-haiku-4-5" },
    });
  });

  it("代理映射在 proxy -> direct -> proxy 切换后保持不变", async () => {
    const user = userEvent.setup();
    renderForm({
      name: "Proxy Provider",
      settingsConfig: {
        env: {
          ANTHROPIC_BASE_URL: "https://api.example.com",
          ANTHROPIC_AUTH_TOKEN: "sk-test",
        },
      },
      meta: {
        claudeDesktopMode: "proxy",
        claudeDesktopModelRoutes: {
          "claude-sonnet-5": { model: "upstream-sonnet" },
        },
      },
    });

    const modeToggle = screen.getByRole("switch", {
      name: "需要模型映射",
    });
    expect(modeToggle).toBeChecked();
    expect(screen.getByDisplayValue("upstream-sonnet")).toBeInTheDocument();

    await user.click(modeToggle);
    expect(modeToggle).not.toBeChecked();
    expect(
      screen.queryByDisplayValue("upstream-sonnet"),
    ).not.toBeInTheDocument();

    await user.click(modeToggle);
    expect(modeToggle).toBeChecked();
    expect(screen.getByDisplayValue("upstream-sonnet")).toBeInTheDocument();
  });

  it("编辑模型映射的菜单显示名时保持输入框焦点", () => {
    renderForm({
      name: "Proxy Provider",
      settingsConfig: {
        env: {
          ANTHROPIC_BASE_URL: "https://api.example.com",
          ANTHROPIC_AUTH_TOKEN: "sk-test",
        },
      },
      meta: {
        claudeDesktopMode: "proxy",
        claudeDesktopModelRoutes: {
          "claude-old": {
            model: "upstream-old",
          },
        },
      },
    });

    // 固定四档（Sonnet / Opus / Fable / Haiku）下有四个菜单显示名输入，取 Sonnet（首个）。
    const input = screen.getAllByPlaceholderText(
      "DeepSeek V4 Pro",
    )[0] as HTMLInputElement;
    input.focus();

    fireEvent.change(input, { target: { value: "DeepSeek V4 Pro" } });

    const currentInput = screen.getAllByPlaceholderText(
      "DeepSeek V4 Pro",
    )[0] as HTMLInputElement;
    expect(currentInput).toHaveValue("DeepSeek V4 Pro");
    expect(document.activeElement).toBe(currentInput);
  });

  it("编辑直连模型列表的模型 ID 时保持输入框焦点", () => {
    renderForm({
      name: "Direct Provider",
      settingsConfig: {
        env: {
          ANTHROPIC_BASE_URL: "https://api.example.com",
          ANTHROPIC_AUTH_TOKEN: "sk-test",
        },
      },
      meta: {
        claudeDesktopMode: "direct",
        claudeDesktopModelRoutes: {
          "claude-old": {
            model: "claude-old",
          },
        },
      },
    });

    const input = screen.getByPlaceholderText(
      "claude-sonnet-4-6",
    ) as HTMLInputElement;
    input.focus();

    fireEvent.change(input, { target: { value: "claude-12345" } });

    const currentInput = screen.getByPlaceholderText(
      "claude-sonnet-4-6",
    ) as HTMLInputElement;
    expect(currentInput).toHaveValue("claude-12345");
    expect(document.activeElement).toBe(currentInput);
  });

  it("代理模式始终渲染 Sonnet / Opus / Fable / Haiku 四档（即使只配了一档）", () => {
    renderForm({
      name: "Proxy Provider",
      settingsConfig: {
        env: {
          ANTHROPIC_BASE_URL: "https://api.example.com",
          ANTHROPIC_AUTH_TOKEN: "sk-test",
        },
      },
      meta: {
        claudeDesktopMode: "proxy",
        claudeDesktopModelRoutes: {
          "claude-sonnet-4-6": { model: "upstream-sonnet" },
        },
      },
    });

    // 固定四档：每档各一个「菜单显示名」输入框，无论初始只配了几档。
    // Haiku 档的占位示例是 "DeepSeek V4 Flash"、其余三档是 "DeepSeek V4 Pro"
    // （见组件的 role-consistent 占位逻辑），故用正则同时匹配两种占位、数满四档。
    expect(
      screen.getAllByPlaceholderText(/DeepSeek V4 (Pro|Flash)/),
    ).toHaveLength(4);
  });

  it("代理模式初始无路由且默认路由未就绪时不渲染空四档", () => {
    // mock 的 getClaudeDesktopDefaultRoutes 返回 []，模拟默认路由尚未就绪。
    // 修复前：normalizeProxyRows([]) 会渲染空行并把 routes.length 撑起来，
    // 永久挡住 seed effect 的默认路由回填。修复后应保持空、等待 seed。
    renderForm({
      name: "Proxy Provider",
      settingsConfig: {
        env: {
          ANTHROPIC_BASE_URL: "https://api.example.com",
          ANTHROPIC_AUTH_TOKEN: "sk-test",
        },
      },
      meta: {
        claudeDesktopMode: "proxy",
        claudeDesktopModelRoutes: {},
      },
    });

    expect(screen.queryAllByPlaceholderText("DeepSeek V4 Pro")).toHaveLength(0);
  });

  it("保存模型映射时补齐固定四档并把留空档回填为 Sonnet 模型", async () => {
    const onSubmit = vi.fn();
    renderForm(
      {
        name: "Proxy Provider",
        settingsConfig: {
          env: {
            ANTHROPIC_BASE_URL: "https://api.example.com",
            ANTHROPIC_AUTH_TOKEN: "sk-test",
          },
        },
        meta: {
          claudeDesktopMode: "proxy",
          claudeDesktopModelRoutes: {
            "claude-old": {
              model: "upstream-old",
            },
          },
        },
      },
      onSubmit,
    );

    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalled());
    const submitted = onSubmit.mock.calls[0][0];
    // claude-old 迁移到 Sonnet；留空的 Opus / Fable / Haiku 回填为 Sonnet 的
    // 上游模型，保证落库四档齐全，子 agent 调用的各档始终可解析。
    expect(submitted.meta.claudeDesktopModelRoutes).toMatchObject({
      "claude-sonnet-5": {
        model: "upstream-old",
        labelOverride: "upstream-old",
      },
      "claude-opus-5": { model: "upstream-old" },
      "claude-fable-5": { model: "upstream-old" },
      "claude-haiku-4-5": { model: "upstream-old" },
    });
    expect(Object.keys(submitted.meta.claudeDesktopModelRoutes).sort()).toEqual(
      [
        "claude-fable-5",
        "claude-haiku-4-5",
        "claude-opus-5",
        "claude-sonnet-5",
      ],
    );
  });

  it("回填空档时继承 Sonnet 的 1M 声明", async () => {
    const onSubmit = vi.fn();
    renderForm(
      {
        name: "Proxy Provider",
        settingsConfig: {
          env: {
            ANTHROPIC_BASE_URL: "https://api.example.com",
            ANTHROPIC_AUTH_TOKEN: "sk-test",
          },
        },
        meta: {
          claudeDesktopMode: "proxy",
          claudeDesktopModelRoutes: {
            "claude-sonnet-4-6": { model: "deepseek-v4-pro", supports1m: true },
          },
        },
      },
      onSubmit,
    );

    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalled());
    const routes = onSubmit.mock.calls[0][0].meta.claudeDesktopModelRoutes;
    // 留空的 Opus / Haiku 回填同一上游模型，1M 声明应与 Sonnet 一致。
    expect(routes["claude-sonnet-5"]).toMatchObject({
      model: "deepseek-v4-pro",
      supports1m: true,
    });
    expect(routes["claude-opus-5"]).toMatchObject({
      model: "deepseek-v4-pro",
      supports1m: true,
    });
    expect(routes["claude-haiku-4-5"]).toMatchObject({
      model: "deepseek-v4-pro",
      supports1m: true,
    });
  });

  it("保存直连模型列表时不会保留旧 route 作为隐藏映射目标", async () => {
    const onSubmit = vi.fn();
    renderForm(
      {
        name: "Direct Provider",
        settingsConfig: {
          env: {
            ANTHROPIC_BASE_URL: "https://api.example.com",
            ANTHROPIC_AUTH_TOKEN: "sk-test",
          },
        },
        meta: {
          claudeDesktopMode: "direct",
          claudeDesktopModelRoutes: {
            "claude-old": {
              model: "claude-old",
            },
          },
        },
      },
      onSubmit,
    );

    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalled());
    const submitted = onSubmit.mock.calls[0][0];
    expect(submitted.meta.claudeDesktopModelRoutes).toMatchObject({
      "claude-sonnet-5": {
        model: "claude-sonnet-5",
      },
    });
  });

  it.each([
    ["default", undefined],
    ["account", codexAccount.id],
    ["pool", undefined],
  ] as const)(
    "Codex OAuth %s 模式保存准确的账号绑定",
    async (mode, accountId) => {
      const onSubmit = vi.fn();
      renderForm(
        {
          name: `Codex ${mode}`,
          category: "third_party",
          settingsConfig: {
            env: {
              ANTHROPIC_BASE_URL: "https://chatgpt.com/backend-api/codex",
            },
          },
          meta: {
            providerType: "codex_oauth",
            apiFormat: "openai_responses",
            claudeDesktopMode: "proxy",
            claudeDesktopModelRoutes: {
              "claude-sonnet-5": { model: "gpt-5.6-sol" },
            },
            authBinding: {
              source: "managed_account",
              authProvider: "codex_oauth",
              mode,
              accountId,
            },
          },
        },
        onSubmit,
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
});
