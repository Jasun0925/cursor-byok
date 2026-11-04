// Failure reproduction for main 2cbd4a58; all network responses and credentials are synthetic.
// Direct plugin boundary only: the candidate moves renewal into the host prepare lifecycle.
import { credentialDraft, refreshAccount, RESOURCE_TYPE } from "../../../server/plugins/build-in/grok-auth/resources.ts";
import { grokProvider } from "../../../server/plugins/build-in/grok-auth/provider.ts";
import type { PluginContext } from "cursor-byok:plugin";
import type { ResourceSnapshot } from "cursor-byok:resource";

const tokenUrl = "https://auth.x.ai/oauth2/token";
const newAccess = "synthetic-new-access";

async function resource(): Promise<ResourceSnapshot> {
  const payload = btoa(JSON.stringify({ sub: "synthetic-user", exp: 1 }));
  const draft = await credentialDraft({
    accessToken: `header.${payload}.signature`,
    refreshToken: "synthetic-refresh-still-valid",
    displayName: "Synthetic account",
  });
  return { id: "synthetic", type: RESOURCE_TYPE, key: draft.key,
    privateData: draft.privateData, state: { status: "ready" } };
}

async function* lines(values: string[]) {
  yield* values;
}

function context(calls: string[]): PluginContext {
  return {
    signal: new AbortController().signal,
    network: {
      fetch: async (url, init) => {
        calls.push(url);
        if (url === tokenUrl) {
          if (!init?.body?.includes("grant_type=refresh_token")) throw new Error("wrong grant");
          return { status: 200, headers: {}, body: JSON.stringify({
            access_token: newAccess, refresh_token: "synthetic-rotated-refresh", expires_in: 1234,
          }) };
        }
        if (init?.headers?.authorization === `Bearer ${newAccess}`) {
          return { status: 200, headers: {}, body: JSON.stringify({
            config: { creditUsagePercent: 0, currentPeriod: { end: 4102444800 } },
          }) };
        }
        return { status: 401, headers: {}, body: "expired access token" };
      },
      stream: async (url, init) => {
        calls.push(url);
        return init?.headers?.authorization === `Bearer ${newAccess}`
          ? { status: 200, headers: {}, lines: lines([
            'data: {"choices":[{"delta":{"content":"OK"},"finish_reason":null}]}',
            'data: {"choices":[{"delta":{},"finish_reason":"stop"}]}', "data: [DONE]",
          ]) }
          : { status: 401, headers: {}, lines: lines(['{"error":"expired access token"}']) };
      },
    },
  };
}

Deno.test("refresh button should renew an expired token before declaring login invalid", async () => {
  const calls: string[] = [];
  const patch = await refreshAccount(await resource(), context(calls));
  console.log(JSON.stringify({ scenario: "refresh", calls, state: patch.state }));
  if (patch.state?.status !== "ready" || !calls.includes(tokenUrl)) {
    throw new Error("renewable login was not refreshed");
  }
});

Deno.test("model call should recover a pre-output 401 using its valid refresh token", async () => {
  const calls: string[] = [];
  const result = await grokProvider.invoke({
    model: { id: "grok-4.6", displayName: "Synthetic Grok" },
    resource: await resource(),
    request: { instructions: "", messages: [{ role: "user", content: [{ type: "text", text: "Hi" }] }],
      tools: [], reasoning: { enabled: false, effort: null }, latency: "standard", maxOutputTokens: null, cacheKey: null },
  }, { emit: () => {} }, context(calls));
  console.log(JSON.stringify({ scenario: "invoke", calls, status: result.status }));
  if (result.status !== "completed" || !calls.includes(tokenUrl)) {
    throw new Error("recoverable 401 was not refreshed");
  }
});
