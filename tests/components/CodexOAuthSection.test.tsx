import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CodexOAuthSection } from "@/components/providers/forms/CodexOAuthSection";
import { AuthCenterPanel } from "@/components/settings/AuthCenterPanel";

const mocks = vi.hoisted(() => ({
  useCodexOauth: vi.fn(),
  renderAccountQuota: vi.fn(),
}));

vi.mock("@/components/providers/forms/hooks/useCodexOauth", () => ({
  useCodexOauth: mocks.useCodexOauth,
}));

vi.mock("@/components/CodexOauthAccountQuota", () => ({
  default: ({ accountId }: { accountId: string }) => {
    mocks.renderAccountQuota(accountId);
    return <div data-testid="account-quota">{accountId}</div>;
  },
}));

vi.mock("@/components/providers/forms/CopilotAuthSection", () => ({
  CopilotAuthSection: () => <div />,
}));

vi.mock("@/components/providers/forms/XaiOAuthSection", () => ({
  XaiOAuthSection: () => <div />,
}));

describe("CodexOAuthSection", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
    mocks.useCodexOauth.mockReturnValue({
      accounts: [
        {
          id: "account-1",
          provider: "codex_oauth",
          login: "user@example.com",
          avatar_url: null,
          authenticated_at: 0,
          is_default: true,
          github_domain: "",
        },
      ],
      defaultAccountId: "account-1",
      hasAnyAccount: true,
      pollingState: "idle",
      deviceCode: null,
      error: null,
      isPolling: false,
      isAddingAccount: false,
      isRemovingAccount: false,
      isSettingDefaultAccount: false,
      isImportingAccounts: false,
      isUpdatingAccount: false,
      addAccount: vi.fn(),
      removeAccount: vi.fn(),
      setDefaultAccount: vi.fn(),
      importAccounts: vi.fn(),
      updateAccount: vi.fn(),
      cancelAuth: vi.fn(),
      logout: vi.fn(),
    });
  });

  it("does not render account quota by default", () => {
    render(<CodexOAuthSection />);

    expect(mocks.renderAccountQuota).not.toHaveBeenCalled();
    expect(screen.queryByTestId("account-quota")).not.toBeInTheDocument();
  });

  it("renders account quota in Auth Center", () => {
    render(<AuthCenterPanel />);

    expect(mocks.renderAccountQuota).toHaveBeenCalledWith("account-1");
    expect(screen.getByTestId("account-quota")).toHaveTextContent("account-1");
  });

  it("selects the default account when switching to fixed-account mode", async () => {
    const user = userEvent.setup();
    const onAccountModeChange = vi.fn();
    const onAccountSelect = vi.fn();
    render(
      <CodexOAuthSection
        accountMode="default"
        onAccountModeChange={onAccountModeChange}
        onAccountSelect={onAccountSelect}
      />,
    );

    await user.click(screen.getByRole("combobox"));
    await user.click(
      screen.getByRole("option", { name: "codexOauth.modeAccount" }),
    );

    expect(onAccountModeChange).toHaveBeenCalledWith("account");
    expect(onAccountSelect).toHaveBeenCalledWith("account-1");
  });

  it("clears a fixed binding when switching to pool mode", async () => {
    const user = userEvent.setup();
    const onAccountModeChange = vi.fn();
    const onAccountSelect = vi.fn();
    render(
      <CodexOAuthSection
        accountMode="account"
        selectedAccountId="account-1"
        onAccountModeChange={onAccountModeChange}
        onAccountSelect={onAccountSelect}
      />,
    );

    await user.click(screen.getAllByRole("combobox")[0]);
    await user.click(
      screen.getByRole("option", { name: "codexOauth.modePool" }),
    );

    expect(onAccountModeChange).toHaveBeenCalledWith("pool");
    expect(onAccountSelect).toHaveBeenCalledWith(null);
  });

  it("labels default mode as native Codex login on the official card", () => {
    render(
      <CodexOAuthSection
        accountMode="default"
        onAccountModeChange={vi.fn()}
        onAccountSelect={vi.fn()}
        nativeCodexLoginDefault
      />,
    );

    expect(
      screen.getByText("codexOauth.modeCodexOfficial"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("codexOauth.modeDescription.codexOfficial"),
    ).toBeInTheDocument();
  });

  it("keeps native Codex login visible without managed accounts", async () => {
    mocks.useCodexOauth.mockReturnValue({
      ...mocks.useCodexOauth(),
      accounts: [],
      defaultAccountId: null,
      hasAnyAccount: false,
    });

    render(
      <CodexOAuthSection
        accountMode="default"
        onAccountModeChange={vi.fn()}
        onAccountSelect={vi.fn()}
        nativeCodexLoginDefault
      />,
    );

    expect(
      screen.getByText("codexOauth.modeCodexOfficial"),
    ).toBeInTheDocument();
    expect(screen.getByText("未导入托管账号")).toBeInTheDocument();
  });
});
