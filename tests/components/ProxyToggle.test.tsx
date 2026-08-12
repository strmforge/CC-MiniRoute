import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProxyToggle } from "@/components/proxy/ProxyToggle";

const useProxyStatusMock = vi.hoisted(() => vi.fn());

vi.mock("@/hooks/useProxyStatus", () => ({
  useProxyStatus: useProxyStatusMock,
}));

describe("ProxyToggle", () => {
  beforeEach(() => {
    useProxyStatusMock.mockReset();
  });

  it("初始代理与接管状态加载完成前禁止切换", () => {
    const proxyState = {
      isRunning: false,
      takeoverStatus: undefined,
      setTakeoverForApp: vi.fn(),
      isPending: false,
      isInitialStatusPending: true,
      status: undefined,
    };
    useProxyStatusMock.mockImplementation(() => proxyState);
    const { rerender } = render(<ProxyToggle activeApp="claude" />);

    expect(screen.getByRole("switch")).toBeDisabled();

    proxyState.isInitialStatusPending = false;
    rerender(<ProxyToggle activeApp="claude" />);

    expect(screen.getByRole("switch")).toBeEnabled();
  });
});
