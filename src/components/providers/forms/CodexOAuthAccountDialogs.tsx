import React from "react";
import { useTranslation } from "react-i18next";
import { FileJson, Loader2, Upload } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type {
  CodexOAuthAccountUpdate,
  CodexOAuthImportOptions,
  CodexOAuthImportResult,
  ManagedAuthAccount,
} from "@/lib/api";

interface CodexOAuthImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  isImporting: boolean;
  onImport: (
    content: string,
    options: CodexOAuthImportOptions,
  ) => Promise<CodexOAuthImportResult>;
}

function toDatetimeLocal(timestamp: number | null | undefined): string {
  if (!timestamp) return "";
  const date = new Date(timestamp);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(timestamp - offset).toISOString().slice(0, 16);
}

function fromDatetimeLocal(value: string): number | null {
  if (!value) return null;
  const timestamp = new Date(value).getTime();
  return Number.isFinite(timestamp) ? timestamp : null;
}

export function CodexOAuthImportDialog({
  open,
  onOpenChange,
  isImporting,
  onImport,
}: CodexOAuthImportDialogProps) {
  const { t } = useTranslation();
  const [content, setContent] = React.useState("");
  const [priority, setPriority] = React.useState(50);
  const [concurrency, setConcurrency] = React.useState(3);
  const [updateExisting, setUpdateExisting] = React.useState(true);
  const [poolEnabled, setPoolEnabled] = React.useState(true);
  const [autoPauseOnExpired, setAutoPauseOnExpired] = React.useState(true);
  const [expiresAt, setExpiresAt] = React.useState("");
  const [proxyUrl, setProxyUrl] = React.useState("");
  const [result, setResult] = React.useState<CodexOAuthImportResult | null>(
    null,
  );
  const [localError, setLocalError] = React.useState<string | null>(null);

  const handleFile = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    try {
      setContent(await file.text());
      setLocalError(null);
      setResult(null);
    } catch (error) {
      setLocalError(error instanceof Error ? error.message : String(error));
    } finally {
      event.target.value = "";
    }
  };

  const handleImport = async () => {
    if (!content.trim()) {
      setLocalError(t("codexOauth.importContentRequired"));
      return;
    }
    setLocalError(null);
    setResult(null);
    try {
      const importResult = await onImport(content, {
        updateExisting,
        priority: Math.max(0, priority),
        concurrency: Math.max(1, concurrency),
        poolEnabled,
        expiresAtMs: fromDatetimeLocal(expiresAt),
        autoPauseOnExpired,
        proxyUrl: proxyUrl.trim() || null,
      });
      setResult(importResult);
      if (importResult.failed === 0) setContent("");
    } catch (error) {
      setLocalError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl" zIndex="nested">
        <DialogHeader>
          <DialogTitle className="text-balance">
            {t("codexOauth.importTitle")}
          </DialogTitle>
          <DialogDescription className="text-pretty">
            {t("codexOauth.importDescription")}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 overflow-y-auto px-6 py-5">
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <Label htmlFor="codex-oauth-import-content">
                {t("codexOauth.importContent")}
              </Label>
              <Label
                htmlFor="codex-oauth-import-file"
                className="inline-flex h-9 cursor-pointer items-center gap-2 rounded-md border border-border-default bg-background px-3 text-sm font-medium hover:bg-muted"
              >
                <FileJson className="size-4" />
                {t("codexOauth.chooseFile")}
              </Label>
              <Input
                id="codex-oauth-import-file"
                type="file"
                accept=".json,.jsonl,.txt,application/json,text/plain"
                className="sr-only"
                onChange={handleFile}
              />
            </div>
            <Textarea
              id="codex-oauth-import-content"
              value={content}
              onChange={(event) => {
                setContent(event.target.value);
                setResult(null);
              }}
              className="min-h-40 resize-y font-mono text-xs"
              placeholder={t("codexOauth.importContentPlaceholder")}
              autoComplete="off"
              spellCheck={false}
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="codex-oauth-import-priority">
                {t("codexOauth.priority")}
              </Label>
              <Input
                id="codex-oauth-import-priority"
                type="number"
                min={0}
                value={priority}
                onChange={(event) =>
                  setPriority(Number(event.target.value) || 0)
                }
                className="tabular-nums"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="codex-oauth-import-concurrency">
                {t("codexOauth.concurrency")}
              </Label>
              <Input
                id="codex-oauth-import-concurrency"
                type="number"
                min={1}
                value={concurrency}
                onChange={(event) =>
                  setConcurrency(Math.max(1, Number(event.target.value) || 1))
                }
                className="tabular-nums"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="codex-oauth-import-expires">
                {t("codexOauth.expiresAt")}
              </Label>
              <Input
                id="codex-oauth-import-expires"
                type="datetime-local"
                value={expiresAt}
                onChange={(event) => setExpiresAt(event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="codex-oauth-import-proxy">
                {t("codexOauth.accountProxy")}
              </Label>
              <Input
                id="codex-oauth-import-proxy"
                value={proxyUrl}
                onChange={(event) => setProxyUrl(event.target.value)}
                placeholder="http://127.0.0.1:7890"
              />
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            {[
              [
                "update-existing",
                t("codexOauth.updateExisting"),
                updateExisting,
                setUpdateExisting,
              ],
              [
                "pool-enabled",
                t("codexOauth.joinPool"),
                poolEnabled,
                setPoolEnabled,
              ],
              [
                "auto-pause",
                t("codexOauth.autoPauseExpired"),
                autoPauseOnExpired,
                setAutoPauseOnExpired,
              ],
            ].map(([id, label, checked, setter]) => (
              <div
                key={String(id)}
                className="flex min-h-11 items-center justify-between gap-3 rounded-md border border-border-default bg-muted/20 px-3"
              >
                <Label htmlFor={`codex-oauth-import-${id}`} className="text-sm">
                  {String(label)}
                </Label>
                <Switch
                  id={`codex-oauth-import-${id}`}
                  checked={Boolean(checked)}
                  onCheckedChange={setter as (checked: boolean) => void}
                />
              </div>
            ))}
          </div>

          {localError && (
            <p className="text-pretty text-sm text-destructive">{localError}</p>
          )}

          {result && (
            <div className="space-y-3 rounded-md border border-border-default bg-muted/20 p-3">
              <div className="flex flex-wrap gap-2 tabular-nums">
                <Badge variant="secondary">
                  {t("codexOauth.importCreated", { count: result.created })}
                </Badge>
                <Badge variant="secondary">
                  {t("codexOauth.importUpdated", { count: result.updated })}
                </Badge>
                <Badge variant="outline">
                  {t("codexOauth.importSkipped", { count: result.skipped })}
                </Badge>
                <Badge variant={result.failed > 0 ? "destructive" : "outline"}>
                  {t("codexOauth.importFailed", { count: result.failed })}
                </Badge>
              </div>
              {result.items.length > 0 && (
                <div className="max-h-40 space-y-1 overflow-y-auto text-xs">
                  {result.items.map((item) => (
                    <div
                      key={`${item.index}-${item.action}`}
                      className="flex items-start justify-between gap-3 py-1"
                    >
                      <span className="min-w-0 truncate text-muted-foreground">
                        #{item.index} {item.name || item.accountId || "-"}
                      </span>
                      <span
                        className={
                          item.action === "failed"
                            ? "shrink-0 text-destructive"
                            : "shrink-0 text-foreground"
                        }
                      >
                        {t(`codexOauth.importAction.${item.action}`)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isImporting}
          >
            {t("common.close", "关闭")}
          </Button>
          <Button type="button" onClick={handleImport} disabled={isImporting}>
            {isImporting ? (
              <Loader2 className="mr-2 size-4 animate-spin" />
            ) : (
              <Upload className="mr-2 size-4" />
            )}
            {t("codexOauth.importAccounts")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface CodexOAuthAccountDialogProps {
  account: ManagedAuthAccount | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  isSaving: boolean;
  onSave: (accountId: string, update: CodexOAuthAccountUpdate) => Promise<void>;
  onReauth: () => void;
}

export function CodexOAuthAccountDialog({
  account,
  open,
  onOpenChange,
  isSaving,
  onSave,
  onReauth,
}: CodexOAuthAccountDialogProps) {
  const { t } = useTranslation();
  const [enabled, setEnabled] = React.useState(account?.enabled ?? true);
  const [poolEnabled, setPoolEnabled] = React.useState(
    account?.pool_enabled ?? true,
  );
  const [priority, setPriority] = React.useState(account?.priority ?? 50);
  const [concurrency, setConcurrency] = React.useState(
    account?.concurrency ?? 3,
  );
  const [expiresAt, setExpiresAt] = React.useState(
    toDatetimeLocal(account?.expires_at_ms),
  );
  const [autoPauseOnExpired, setAutoPauseOnExpired] = React.useState(
    account?.auto_pause_on_expired ?? true,
  );
  const [proxyUrl, setProxyUrl] = React.useState(account?.proxy_url ?? "");
  const [localError, setLocalError] = React.useState<string | null>(null);

  if (!account) return null;

  const handleSave = async () => {
    setLocalError(null);
    try {
      const expiresAtMs = fromDatetimeLocal(expiresAt);
      await onSave(account.id, {
        enabled,
        poolEnabled,
        priority: Math.max(0, priority),
        concurrency: Math.max(1, concurrency),
        ...(expiresAtMs === null ? { clearExpiresAt: true } : { expiresAtMs }),
        autoPauseOnExpired,
        ...(proxyUrl.trim()
          ? { proxyUrl: proxyUrl.trim() }
          : { clearProxyUrl: true }),
      });
      onOpenChange(false);
    } catch (error) {
      setLocalError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg" zIndex="nested">
        <DialogHeader>
          <DialogTitle className="truncate">{account.login}</DialogTitle>
          <DialogDescription className="text-pretty">
            {t("codexOauth.accountSettings")}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 overflow-y-auto px-6 py-5">
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="flex min-h-11 items-center justify-between gap-3 rounded-md border border-border-default bg-muted/20 px-3">
              <Label htmlFor="codex-oauth-account-enabled">
                {t("codexOauth.accountEnabled")}
              </Label>
              <Switch
                id="codex-oauth-account-enabled"
                checked={enabled}
                onCheckedChange={setEnabled}
              />
            </div>
            <div className="flex min-h-11 items-center justify-between gap-3 rounded-md border border-border-default bg-muted/20 px-3">
              <Label htmlFor="codex-oauth-account-pool">
                {t("codexOauth.joinPool")}
              </Label>
              <Switch
                id="codex-oauth-account-pool"
                checked={poolEnabled}
                onCheckedChange={setPoolEnabled}
              />
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="codex-oauth-account-priority">
                {t("codexOauth.priority")}
              </Label>
              <Input
                id="codex-oauth-account-priority"
                type="number"
                min={0}
                value={priority}
                onChange={(event) =>
                  setPriority(Number(event.target.value) || 0)
                }
                className="tabular-nums"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="codex-oauth-account-concurrency">
                {t("codexOauth.concurrency")}
              </Label>
              <Input
                id="codex-oauth-account-concurrency"
                type="number"
                min={1}
                value={concurrency}
                onChange={(event) =>
                  setConcurrency(Math.max(1, Number(event.target.value) || 1))
                }
                className="tabular-nums"
              />
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="codex-oauth-account-expires">
              {t("codexOauth.expiresAt")}
            </Label>
            <Input
              id="codex-oauth-account-expires"
              type="datetime-local"
              value={expiresAt}
              onChange={(event) => setExpiresAt(event.target.value)}
            />
          </div>

          <div className="flex min-h-11 items-center justify-between gap-3 rounded-md border border-border-default bg-muted/20 px-3">
            <Label htmlFor="codex-oauth-account-auto-pause">
              {t("codexOauth.autoPauseExpired")}
            </Label>
            <Switch
              id="codex-oauth-account-auto-pause"
              checked={autoPauseOnExpired}
              onCheckedChange={setAutoPauseOnExpired}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="codex-oauth-account-proxy">
              {t("codexOauth.accountProxy")}
            </Label>
            <Input
              id="codex-oauth-account-proxy"
              value={proxyUrl}
              onChange={(event) => setProxyUrl(event.target.value)}
              placeholder="http://127.0.0.1:7890"
            />
          </div>

          {!account.renewable && (
            <p className="text-pretty text-sm text-amber-600 dark:text-amber-400">
              {t("codexOauth.accessOnlyWarning")}
            </p>
          )}
          {account.last_error && (
            <p className="text-pretty text-sm text-destructive">
              {account.last_error}
            </p>
          )}
          {localError && (
            <p className="text-pretty text-sm text-destructive">{localError}</p>
          )}
        </div>

        <DialogFooter className="sm:justify-between">
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              onOpenChange(false);
              onReauth();
            }}
            disabled={isSaving}
          >
            {t("codexOauth.reauthenticate")}
          </Button>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={isSaving}
            >
              {t("common.cancel", "取消")}
            </Button>
            <Button type="button" onClick={handleSave} disabled={isSaving}>
              {isSaving && <Loader2 className="mr-2 size-4 animate-spin" />}
              {t("common.save", "保存")}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
