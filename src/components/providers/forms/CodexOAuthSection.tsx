import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Loader2,
  LogOut,
  Copy,
  Check,
  ExternalLink,
  Plus,
  Settings2,
  Upload,
  X,
  Sparkles,
  User,
} from "lucide-react";
import { useCodexOauth } from "./hooks/useCodexOauth";
import { copyText } from "@/lib/clipboard";
import CodexOauthAccountQuota from "@/components/CodexOauthAccountQuota";
import type { ManagedAuthAccount } from "@/lib/api";
import {
  CodexOAuthAccountDialog,
  CodexOAuthImportDialog,
} from "./CodexOAuthAccountDialogs";

interface CodexOAuthSectionProps {
  className?: string;
  /** 是否展示每个账号的订阅额度 */
  showAccountQuota?: boolean;
  /** 当前选中的 ChatGPT 账号 ID */
  selectedAccountId?: string | null;
  /** 账号选择回调 */
  onAccountSelect?: (accountId: string | null) => void;
  /** Provider 使用账号的方式 */
  accountMode?: "default" | "account" | "pool";
  /** Provider 使用方式切换回调 */
  onAccountModeChange?: (mode: "default" | "account" | "pool") => void;
  /** 默认模式表示继续使用 Codex 自己的官方登录，而非托管默认账号 */
  nativeCodexLoginDefault?: boolean;
  /** 是否开启 Codex FAST mode */
  fastModeEnabled?: boolean;
  /** FAST mode 切换回调 */
  onFastModeChange?: (enabled: boolean) => void;
}

/**
 * Codex OAuth 认证区块
 *
 * 通过 OpenAI Device Code 流程登录 ChatGPT Plus/Pro 账号，
 * 用于将 Claude Code 请求反代到 Codex 后端 API。
 */
export const CodexOAuthSection: React.FC<CodexOAuthSectionProps> = ({
  className,
  showAccountQuota = false,
  selectedAccountId,
  onAccountSelect,
  accountMode,
  onAccountModeChange,
  nativeCodexLoginDefault = false,
  fastModeEnabled = false,
  onFastModeChange,
}) => {
  const { t } = useTranslation();
  const [copied, setCopied] = React.useState(false);
  const [importOpen, setImportOpen] = React.useState(false);
  const [editingAccount, setEditingAccount] =
    React.useState<ManagedAuthAccount | null>(null);

  const {
    accounts,
    defaultAccountId,
    hasAnyAccount,
    pollingState,
    deviceCode,
    error,
    isPolling,
    isAddingAccount,
    isRemovingAccount,
    isSettingDefaultAccount,
    isImportingAccounts,
    isUpdatingAccount,
    addAccount,
    removeAccount,
    setDefaultAccount,
    importAccounts,
    updateAccount,
    cancelAuth,
    logout,
  } = useCodexOauth();

  const copyUserCode = async () => {
    if (deviceCode?.user_code) {
      await copyText(deviceCode.user_code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleAccountSelect = (value: string) => {
    onAccountSelect?.(value === "none" ? null : value);
  };

  const handleAccountModeChange = (value: string) => {
    const mode = value as "default" | "account" | "pool";
    onAccountModeChange?.(mode);
    if (mode === "account") {
      onAccountSelect?.(
        selectedAccountId || defaultAccountId || accounts[0]?.id || null,
      );
    } else {
      onAccountSelect?.(null);
    }
  };

  const statusLabel = (account: ManagedAuthAccount) =>
    t(`codexOauth.status.${account.status || "active"}`);

  const handleRemoveAccount = (accountId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    removeAccount(accountId);
    if (selectedAccountId === accountId) {
      onAccountSelect?.(null);
    }
  };

  return (
    <div className={`space-y-4 ${className || ""}`}>
      {/* 认证状态标题 */}
      <div className="flex items-center justify-between">
        <Label>
          {nativeCodexLoginDefault
            ? t("codexOauth.managedAccounts", "CC Switch 托管账号")
            : t("codexOauth.authStatus", "认证状态")}
        </Label>
        <Badge
          variant={hasAnyAccount ? "default" : "secondary"}
          className={hasAnyAccount ? "bg-green-500 hover:bg-green-600" : ""}
        >
          {hasAnyAccount
            ? t("codexOauth.accountCount", {
                count: accounts.length,
                defaultValue: `${accounts.length} 个账号`,
              })
            : nativeCodexLoginDefault
              ? t("codexOauth.noManagedAccounts", "未导入托管账号")
              : t("codexOauth.notAuthenticated", "未认证")}
        </Badge>
      </div>

      {/* Provider 账号使用方式 */}
      {(hasAnyAccount || nativeCodexLoginDefault) &&
        onAccountSelect &&
        onAccountModeChange && (
          <div className="space-y-2">
            <Label className="text-sm text-muted-foreground">
              {t("codexOauth.accountMode")}
            </Label>
            <Select
              value={accountMode || "default"}
              onValueChange={handleAccountModeChange}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="default">
                  {t(
                    nativeCodexLoginDefault
                      ? "codexOauth.modeCodexOfficial"
                      : "codexOauth.modeDefault",
                  )}
                </SelectItem>
                <SelectItem value="account" disabled={!hasAnyAccount}>
                  {t("codexOauth.modeAccount")}
                </SelectItem>
                <SelectItem value="pool" disabled={!hasAnyAccount}>
                  {t("codexOauth.modePool")}
                </SelectItem>
              </SelectContent>
            </Select>
            <p className="text-pretty text-xs text-muted-foreground">
              {accountMode === "default" || !accountMode
                ? t(
                    nativeCodexLoginDefault
                      ? "codexOauth.modeDescription.codexOfficial"
                      : "codexOauth.modeDescription.default",
                  )
                : t(`codexOauth.modeDescription.${accountMode}`)}
            </p>
          </div>
        )}

      {/* 固定账号选择器 */}
      {hasAnyAccount &&
        onAccountSelect &&
        (!onAccountModeChange || accountMode === "account") && (
          <div className="space-y-2">
            <Label className="text-sm text-muted-foreground">
              {t("codexOauth.selectAccount", "选择账号")}
            </Label>
            <Select
              value={selectedAccountId || (onAccountModeChange ? "" : "none")}
              onValueChange={handleAccountSelect}
            >
              <SelectTrigger>
                <SelectValue
                  placeholder={t(
                    "codexOauth.selectAccountPlaceholder",
                    "选择一个 ChatGPT 账号",
                  )}
                />
              </SelectTrigger>
              <SelectContent>
                {!onAccountModeChange && (
                  <SelectItem value="none">
                    <span className="text-muted-foreground">
                      {t("codexOauth.useDefaultAccount", "使用默认账号")}
                    </span>
                  </SelectItem>
                )}
                {accounts.map((account) => (
                  <SelectItem key={account.id} value={account.id}>
                    <div className="flex items-center gap-2">
                      <User className="h-4 w-4 text-muted-foreground" />
                      <span>{account.login}</span>
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}

      {onFastModeChange && (
        <div className="flex items-center justify-between rounded-md border bg-muted/30 p-3">
          <div className="space-y-1 pr-4">
            <Label className="text-sm font-medium">
              {t("codexOauth.fastMode", "FAST mode")}
            </Label>
            <p className="text-xs text-muted-foreground">
              {t("codexOauth.fastModeDescription", {
                defaultValue:
                  'Send service_tier="priority" for lower latency. Turn it off if the ChatGPT Codex backend rejects the parameter.',
              })}
            </p>
          </div>
          <Switch
            checked={fastModeEnabled}
            onCheckedChange={onFastModeChange}
            aria-label={t("codexOauth.fastMode", "FAST mode")}
          />
        </div>
      )}

      {/* 已登录账号列表 */}
      {hasAnyAccount && (
        <div className="space-y-2">
          <Label className="text-sm text-muted-foreground">
            {t("codexOauth.loggedInAccounts", "已登录账号")}
          </Label>
          <div className="space-y-1">
            {accounts.map((account) => (
              <div
                key={account.id}
                className="space-y-2 p-2 rounded-md border bg-muted/30"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0 space-y-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <User className="h-5 w-5 text-muted-foreground" />
                      <span className="min-w-0 truncate text-sm font-medium">
                        {account.login}
                      </span>
                      {defaultAccountId === account.id && (
                        <Badge variant="secondary" className="text-xs">
                          {t("codexOauth.defaultAccount", "默认")}
                        </Badge>
                      )}
                      {selectedAccountId === account.id && (
                        <Badge variant="outline" className="text-xs">
                          {t("codexOauth.selected", "已选中")}
                        </Badge>
                      )}
                      <Badge
                        variant={
                          account.status === "active" ? "outline" : "secondary"
                        }
                        className="text-xs"
                      >
                        {statusLabel(account)}
                      </Badge>
                    </div>
                    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 pl-7 text-xs text-muted-foreground tabular-nums">
                      {account.plan_type && <span>{account.plan_type}</span>}
                      <span>
                        {t("codexOauth.priorityShort", {
                          value: account.priority ?? 50,
                        })}
                      </span>
                      <span>
                        {t("codexOauth.concurrencyShort", {
                          value: account.concurrency ?? 3,
                        })}
                      </span>
                      <span>
                        {account.renewable
                          ? t("codexOauth.renewable")
                          : t("codexOauth.accessOnly")}
                      </span>
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <Switch
                      checked={account.pool_enabled ?? true}
                      onCheckedChange={(checked) =>
                        void updateAccount(account.id, { poolEnabled: checked })
                      }
                      disabled={isUpdatingAccount}
                      aria-label={t("codexOauth.joinPool")}
                    />
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground"
                      onClick={() => setEditingAccount(account)}
                      disabled={isUpdatingAccount}
                      title={t("codexOauth.editAccount")}
                      aria-label={t("codexOauth.editAccount")}
                    >
                      <Settings2 className="h-4 w-4" />
                    </Button>
                    {defaultAccountId !== account.id && (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-xs text-muted-foreground"
                        onClick={() => setDefaultAccount(account.id)}
                        disabled={isSettingDefaultAccount}
                      >
                        {t("codexOauth.setAsDefault", "设为默认")}
                      </Button>
                    )}
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground hover:text-red-500"
                      onClick={(e) => handleRemoveAccount(account.id, e)}
                      disabled={isRemovingAccount}
                      title={t("codexOauth.removeAccount", "移除账号")}
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
                {showAccountQuota && (
                  <CodexOauthAccountQuota accountId={account.id} />
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 未认证 - 登录/导入按钮 */}
      {!hasAnyAccount && pollingState === "idle" && (
        <div className="grid gap-2 sm:grid-cols-2">
          <Button type="button" onClick={addAccount} variant="outline">
            <Sparkles className="mr-2 h-4 w-4" />
            {t("codexOauth.loginWithChatGPT", "使用 ChatGPT 登录")}
          </Button>
          <Button
            type="button"
            onClick={() => setImportOpen(true)}
            variant="outline"
          >
            <Upload className="mr-2 h-4 w-4" />
            {t("codexOauth.importAccounts")}
          </Button>
        </div>
      )}

      {/* 已有账号 - 添加更多按钮 */}
      {hasAnyAccount && pollingState === "idle" && (
        <div className="grid gap-2 sm:grid-cols-2">
          <Button
            type="button"
            onClick={addAccount}
            variant="outline"
            disabled={isAddingAccount}
          >
            <Plus className="mr-2 h-4 w-4" />
            {t("codexOauth.addAnotherAccount", "添加其他账号")}
          </Button>
          <Button
            type="button"
            onClick={() => setImportOpen(true)}
            variant="outline"
            disabled={isImportingAccounts}
          >
            <Upload className="mr-2 h-4 w-4" />
            {t("codexOauth.importAccounts")}
          </Button>
        </div>
      )}

      {/* 轮询中状态 */}
      {isPolling && deviceCode && (
        <div className="space-y-3 p-4 rounded-lg border border-border bg-muted/50">
          <div className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            {t("codexOauth.waitingForAuth", "等待授权中...")}
          </div>

          <div className="text-center">
            <p className="text-xs text-muted-foreground mb-1">
              {t("codexOauth.enterCode", "在浏览器中输入以下代码：")}
            </p>
            <div className="flex items-center justify-center gap-2">
              <code className="text-2xl font-mono font-bold tracking-wider bg-background px-4 py-2 rounded border">
                {deviceCode.user_code}
              </code>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                onClick={copyUserCode}
                title={t("codexOauth.copyCode", "复制代码")}
              >
                {copied ? (
                  <Check className="h-4 w-4 text-green-500" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>

          <div className="text-center">
            <a
              href={deviceCode.verification_uri}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 text-sm text-blue-500 hover:underline"
            >
              {deviceCode.verification_uri}
              <ExternalLink className="h-3 w-3" />
            </a>
          </div>

          <div className="text-center">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={cancelAuth}
            >
              {t("common.cancel", "取消")}
            </Button>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {pollingState === "error" && error && (
        <div className="space-y-2">
          <p className="text-sm text-red-500">{error}</p>
          <div className="flex gap-2">
            <Button
              type="button"
              onClick={addAccount}
              variant="outline"
              size="sm"
            >
              {t("codexOauth.retry", "重试")}
            </Button>
            <Button
              type="button"
              onClick={cancelAuth}
              variant="ghost"
              size="sm"
            >
              {t("common.cancel", "取消")}
            </Button>
          </div>
        </div>
      )}

      {/* 注销所有账号 */}
      {hasAnyAccount && accounts.length > 1 && (
        <Button
          type="button"
          variant="outline"
          onClick={logout}
          className="w-full text-red-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-950"
        >
          <LogOut className="mr-2 h-4 w-4" />
          {t("codexOauth.logoutAll", "注销所有账号")}
        </Button>
      )}

      <CodexOAuthImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        isImporting={isImportingAccounts}
        onImport={importAccounts}
      />
      <CodexOAuthAccountDialog
        key={editingAccount?.id || "no-account"}
        account={editingAccount}
        open={editingAccount !== null}
        onOpenChange={(open) => !open && setEditingAccount(null)}
        isSaving={isUpdatingAccount}
        onSave={updateAccount}
        onReauth={addAccount}
      />
    </div>
  );
};

export default CodexOAuthSection;
