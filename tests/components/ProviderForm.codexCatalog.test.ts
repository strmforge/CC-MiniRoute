import { describe, expect, it } from "vitest";
import {
  normalizeCodexCatalogModelsForSave,
  resolveCodexDefaultModelForSave,
} from "@/components/providers/forms/ProviderForm";

describe("ProviderForm Codex catalog helpers", () => {
  it("normalizes catalog rows and removes empty or duplicate models", () => {
    expect(
      normalizeCodexCatalogModelsForSave([
        { model: " deepseek-v4-flash ", displayName: " DeepSeek " },
        { model: "deepseek-v4-flash", displayName: "Duplicate" },
        { model: "", displayName: "Empty" },
        { model: "kimi-k2", contextWindow: "128000 tokens" },
      ]),
    ).toEqual([
      { model: "deepseek-v4-flash", displayName: "DeepSeek" },
      { model: "kimi-k2", contextWindow: 128000 },
    ]);
  });

  it("keeps the TOML model when it exists in the catalog even if it is not first", () => {
    expect(
      resolveCodexDefaultModelForSave('model = "MiniMax-M3"\n', [
        { model: "MiniMax-M2.7", displayName: "MiniMax M2.7" },
        { model: "MiniMax-M3", displayName: "MiniMax M3" },
      ]),
    ).toBe("MiniMax-M3");
  });

  it("falls back to the first catalog model when TOML points to a missing model", () => {
    expect(
      resolveCodexDefaultModelForSave('model = "old-model"\n', [
        { model: "MiniMax-M3", displayName: "MiniMax M3" },
        { model: "MiniMax-M2.7", displayName: "MiniMax M2.7" },
      ]),
    ).toBe("MiniMax-M3");
  });
});
