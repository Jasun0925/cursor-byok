import type { ModelType } from "../api";
import deepseekIcon from "../assets/provider-icons/deepseek.svg";
import geminiIcon from "../assets/provider-icons/gemini.svg";
import huoshanIcon from "../assets/provider-icons/huoshan.png";
import kimiIcon from "../assets/provider-icons/kimi.svg";
import minimaxIcon from "../assets/provider-icons/minimax.svg";
import zhipuIcon from "../assets/provider-icons/zhipu.svg";
import { defaultCustomHeaders } from "./modelDefaults";

export interface ModelPresetEntry {
  model_id: string;
  display_name: string;
  context_window_tokens: number | null;
  max_output_tokens: number | null;
}

/** 一个服务商在某种协议（anthropic / openai）下的接入端点 */
export interface ModelPresetEndpoint {
  baseUrl: string;
  /** true 时 baseUrl 即完整请求 URL；false 时由请求协议追加标准端点路径 */
  useFullUrl: boolean;
  /** openai 协议的请求端点（useFullUrl 为 true 时忽略） */
  openaiEndpoint: string;
  /** 非空时启用自定义 Headers（claude-cli 伪装头） */
  customHeaders: Record<string, string> | null;
}

export interface ModelPreset {
  key: string;
  name: string;
  icon: string;
  keyHint: string;
  /** 服务商可以只提供其中一种协议；不支持的协议使用 null。 */
  endpoints: { anthropic: ModelPresetEndpoint | null; openai: ModelPresetEndpoint | null };
  models: ModelPresetEntry[];
}

const entry = (
  modelId: string,
  displayName: string,
  contextWindowTokens: number | null,
  maxOutputTokens: number | null,
): ModelPresetEntry => ({
  model_id: modelId,
  display_name: displayName,
  context_window_tokens: contextWindowTokens,
  max_output_tokens: maxOutputTokens,
});

const claudeHeaders = { ...defaultCustomHeaders };
/** anthropic 协议：填 Base URL，自动追加 /v1/messages */
const anthropic = (baseUrl: string): ModelPresetEndpoint => ({ baseUrl, useFullUrl: false, openaiEndpoint: "", customHeaders: claudeHeaders });
/** openai 协议：填 Base URL，自动追加 /v1/chat/completions */
const openaiChat = (baseUrl: string): ModelPresetEndpoint => ({ baseUrl, useFullUrl: false, openaiEndpoint: "/v1/chat/completions", customHeaders: null });
/** openai 协议：路径不规则，直接给完整请求 URL */
const openaiFullUrl = (url: string): ModelPresetEndpoint => ({ baseUrl: url, useFullUrl: true, openaiEndpoint: "/v1/chat/completions", customHeaders: null });

export const modelPresets: ModelPreset[] = [
  {
    key: "gemini",
    name: "Google Gemini（OpenAI 兼容）",
    icon: geminiIcon,
    keyHint: "Google AI Studio → Gemini API Key；使用 OpenAI 兼容接口（原生 generateContent 暂不支持）",
    endpoints: {
      anthropic: null,
      openai: openaiFullUrl("https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"),
    },
    models: [entry("gemini-3.8-flash", "Gemini 3.8 Flash", 1000000, 65536)],
  },
  {
    key: "zhipu",
    name: "智谱 GLM",
    icon: zhipuIcon,
    keyHint: "bigmodel.cn → GLM Coding Plan → API Key（套餐 Key 与普通 Key 不通用）",
    endpoints: {
      anthropic: anthropic("https://open.bigmodel.cn/api/anthropic"),
      openai: openaiFullUrl("https://open.bigmodel.cn/api/coding/paas/v4/chat/completions"),
    },
    models: [
      entry("glm-5.3", "GLM 5.3", 1000000, 65536),
      entry("glm-5.2", "GLM 5.2", 200000, 32768),
      entry("glm-4.7", "GLM 4.7", 200000, 32768),
    ],
  },
  {
    key: "kimi",
    name: "Kimi (Moonshot)",
    icon: kimiIcon,
    keyHint: "Kimi Code 编程套餐页获取 API Key（api.kimi.com/coding 端点）",
    endpoints: {
      anthropic: anthropic("https://api.kimi.com/coding"),
      openai: openaiChat("https://api.kimi.com/coding"),
    },
    models: [
      entry("k3", "Kimi K3", 1048576, 65536),
      entry("kimi-for-coding", "K2.7 Coding", 262144, 32768),
    ],
  },
  {
    key: "moonshot",
    name: "Moonshot Kimi API",
    icon: kimiIcon,
    keyHint: "platform.moonshot.cn → API Key；Kimi K3 使用 kimi-k3 模型标识",
    endpoints: {
      anthropic: null,
      openai: openaiChat("https://api.moonshot.cn"),
    },
    models: [entry("kimi-k3", "Kimi K3", 1048576, 65536)],
  },
  {
    key: "deepseek",
    name: "DeepSeek",
    icon: deepseekIcon,
    keyHint: "platform.deepseek.com → API Keys",
    endpoints: {
      anthropic: anthropic("https://api.deepseek.com/anthropic"),
      openai: openaiChat("https://api.deepseek.com"),
    },
    models: [
      entry("deepseek-v4-pro", "DeepSeek V4 Pro", 1000000, 65536),
      entry("deepseek-v4-flash", "DeepSeek V4 Flash", 1000000, null),
    ],
  },
  {
    key: "volcengine",
    name: "火山引擎方舟",
    icon: huoshanIcon,
    keyHint: "火山方舟 Coding Plan（ark-code-latest 路由多款代码模型）",
    endpoints: {
      anthropic: anthropic("https://ark.cn-beijing.volces.com/api/coding"),
      openai: openaiFullUrl("https://ark.cn-beijing.volces.com/api/coding/v3/chat/completions"),
    },
    models: [entry("ark-code-latest", "Ark Code Latest", 256000, 32768)],
  },
  {
    key: "minimax",
    name: "MiniMax",
    icon: minimaxIcon,
    keyHint: "platform.minimaxi.com → 订阅 Coding Plan → API Key",
    endpoints: {
      anthropic: anthropic("https://api.minimaxi.com/anthropic"),
      openai: { baseUrl: "https://api.minimaxi.com", useFullUrl: false, openaiEndpoint: "/v1/responses", customHeaders: null },
    },
    models: [
      entry("MiniMax-M3", "MiniMax M3", 1000000, 65536),
      entry("MiniMax-M2.7", "MiniMax M2.7", 205000, 32768),
    ],
  },
];

export const trimTrailingSlash = (url: string) => url.replace(/\/+$/, "");

export const presetEndpoint = (preset: ModelPreset, type: ModelType): ModelPresetEndpoint | null => preset.endpoints[type];
