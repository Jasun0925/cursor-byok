# Cursor 思考展示检查 · 2026-10-03

## 结论

截图中英文标题由供应商 OpenAI Chat 流的 `reasoning_content` 原样提供：`**Formulating concise summary with citations**`。普通 `content` 中没有这句话。BYOK 转为 Cursor `ThinkingDelta`，没有误放到正文或把工具进度当作思考。

原始 412 个 Cursor SSE 帧中，第 43 帧是上述 ThinkingDelta，第 44 帧 ThinkingCompleted 的 thinking_duration_ms=17；持久化 ThinkingMessage 同样记为 17 ms。

## 计时限制

此次模型调用收到首块响应的记录偏移为 5984 ms，思考摘要只用一个增量发来，接着很快进入正文。当前代码从接收到 ThinkingStart 开始计时，到 ThinkingEnd 截止，结果为 17 ms；它衡量本地思考内容流的持续时间，不能代表模型真实推理耗时。首块前等待包含供应商处理和网络，不应全部算作思考。

因此标题文字与区域映射正确；“Thought briefly”是 Cursor 根据事件作出的显示，不能据此判定模型没有认真思考或推理能力异常。这里有计时展示的准确性限制，但与 #477 子代理准入修复无关。

## 对照路径

```text
供应商 delta.reasoning_content
→ server/src/provider/openai_chat.rs：ThinkingStart / ThinkingDelta
→ server/src/run/model_cycle.rs：记录本地增量流起止时间
→ server/src/cursor/conversation/output/narration.rs：ThinkingDelta 转发
→ server/src/cursor/protocol/events.rs：ThinkingCompleted 时长
→ Cursor：思考折叠区和 Thought briefly 标签
```

本次只读检查已有证据，未重启测试环境，未修改生产代码。结构化证据见 `2026-10-03-issue477-thinking-check.json`。
