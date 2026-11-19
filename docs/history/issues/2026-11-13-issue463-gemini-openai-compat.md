# Issue #463 / #184：Gemini OpenAI 兼容预设

上游 Issue [#463](https://github.com/leookun/cursor-byok/issues/463) 只提供了一个鉴权提示：Gemini 原生 API 使用 `x-goog-api-key`；[#184](https://github.com/leookun/cursor-byok/issues/184) 的现场错误则是把 Gemini 配置到现有 OpenAI/Anthropic 适配器后得到 405。这不足以支持直接把 Cursor 的 OpenAI 请求体发送到 Gemini 原生 `generateContent` 端点，因为两者的 URL、请求体和流式响应格式不同。

Google 同时提供 OpenAI 兼容入口：

```text
https://generativelanguage.googleapis.com/v1beta/openai/chat/completions
```

该入口使用 `Authorization: Bearer <Gemini API key>`，与现有 OpenAI Chat provider 完全匹配。因此本次增加一个桌面预设：

```text
Google Gemini（OpenAI 兼容）
  请求协议：OpenAI Chat Completions
  完整请求 URL：上述兼容入口
  默认模型：gemini-3.8-flash
```

预设只降低配置成本，不声称实现 Gemini 原生协议。用户可以点击“获取模型”选择账号当前可用的其他 Gemini 模型；需要原生 `generateContent`、文件 API、Grounding 等 Gemini 专有能力时，仍需要独立的 Gemini provider，当前不在这个小修复范围内。

## 验证

- `npm --prefix apps/desktop run check`
- 预设数据检查：协议为 `openai`、使用完整请求 URL、没有 Anthropic 端点；API Key 由现有 OpenAI provider 以 Bearer 发送。
- 未使用真实 Gemini API Key；没有向 Google 发起真实请求。
