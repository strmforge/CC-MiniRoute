# Codex non-GPT model bridge

CC MiniRoute has two different ways to route Codex requests:

- **Failover**: best for OpenAI Official and GPT-compatible providers. Codex still sees the GPT model name, and MiniRoute changes the upstream provider when a request fails.
- **Non-GPT model bridge**: best for providers that expose their own model names, such as MiniMax, DeepSeek, Qwen, GLM, Moonshot, Kimi, Doubao, and other OpenAI-compatible Chat APIs.

The bridge is off by default. Enable it in:

`Settings -> Routing -> Auto Failover -> Codex non-GPT model bridge`

When enabled, MiniRoute writes non-GPT entries from Codex provider `modelCatalog` into Codex's generated `model_catalog_json`. GPT/OpenAI-family names such as `gpt-5.5`, `gpt-5.4`, and `gpt-5.4-mini` are deliberately skipped.

## Recommended usage

Use failover for providers that serve GPT-compatible traffic:

- OpenAI Official
- Third-party gateways that expose GPT family names
- Any provider where Codex should keep using `gpt-*` model names

Use the bridge for non-GPT provider models:

- MiniMax M3 / M2.7
- DeepSeek models
- Qwen / GLM / Moonshot / Kimi
- Other providers where the actual model name is not a GPT family name

If both are enabled, keep the mental model simple:

- GPT family traffic follows failover order.
- Non-GPT model selections are exposed through the bridge and routed by model name.

After changing the bridge switch or model catalog, restart or refresh Codex if the model list does not update immediately. Tool-call stability still depends on the upstream model and provider.

Turning the bridge off removes MiniRoute's generated catalog reference from Codex on the next live sync. It does not delete your saved provider `modelCatalog`; it only stops projecting it into Codex.
