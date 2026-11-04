import type { JsonValue, PluginContext } from "cursor-byok:plugin";
import type { ResourceSnapshot } from "cursor-byok:resource";
import { prepareAccount, TOKEN_URL } from "./credentials.ts";
import { codexProvider } from "./provider.ts";
import { RESOURCE_TYPE } from "./resources.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function jwt(payload: Record<string, unknown>): string {
  const encoded = btoa(JSON.stringify(payload)).replace(/=/g, "").replace(/\+/g, "-").replace(
    /\//g,
    "_",
  );
  return `header.${encoded}.signature`;
}

function snapshot(
  privateData: JsonValue,
  state: ResourceSnapshot["state"] = { status: "ready" },
): ResourceSnapshot {
  return { id: "account-1", type: RESOURCE_TYPE, key: "codex:account-1", privateData, state };
}

Deno.test("Codex prepares expiring credentials and returns rotated tokens for persistence", async () => {
  const oldToken = jwt({ exp: Math.floor(Date.now() / 1000) - 1 });
  const newToken = jwt({ exp: Math.floor(Date.now() / 1000) + 3600 });
  const resource = snapshot({
    accessToken: oldToken,
    refreshToken: "refresh-before",
    accountId: "account-1",
    displayName: "test",
    quota: null,
  });
  let calls = 0;
  const context = {
    signal: new AbortController().signal,
    network: {
      fetch: async (url: string, init?: { headers?: Record<string, string>; body?: string }) => {
        calls += 1;
        assert(url === TOKEN_URL, `unexpected refresh URL: ${url}`);
        assert(
          init?.headers?.["content-type"] === "application/x-www-form-urlencoded",
          "Codex token endpoint requires form encoding",
        );
        const body = new URLSearchParams(init?.body);
        assert(body.get("grant_type") === "refresh_token", "missing refresh grant");
        assert(body.get("client_id") === "app_EMoamEEZ73f0CkXaXp7hrann", "wrong client ID");
        assert(body.get("refresh_token") === "refresh-before", "wrong refresh token");
        return {
          status: 200,
          headers: {},
          body: JSON.stringify({
            access_token: newToken,
            refresh_token: "refresh-after",
            expires_in: "3600",
          }),
        };
      },
      stream: () => Promise.reject(new Error("stream is not expected")),
    },
  } satisfies PluginContext;

  const patch = await prepareAccount(resource, null, context);
  assert(calls === 1, `expected one refresh request, got ${calls}`);
  assert(patch?.state?.status === "ready", "refreshed account should be ready");
  const privateData = patch.privateData as Record<string, JsonValue>;
  assert(privateData.accessToken === newToken, "new access token missing from patch");
  assert(privateData.refreshToken === "refresh-after", "rotated refresh token missing from patch");
  assert(
    typeof privateData.expiresAtMs === "number" && privateData.expiresAtMs > Date.now(),
    "new expiry missing from patch",
  );
});

Deno.test("Codex turns a rejected provider token into one host-managed auth retry", async () => {
  const events: unknown[] = [];
  const result = await codexProvider.invoke(
    {
      model: { id: "gpt-test", displayName: "GPT Test" },
      resource: snapshot({
        accessToken: "expired-access",
        refreshToken: "refresh-token",
        accountId: "account-1",
        displayName: "test",
        quota: null,
      }),
      request: {
        instructions: "test",
        messages: [{ role: "user", content: [{ type: "text", text: "hello" }] }],
        tools: [],
        reasoning: { enabled: false, effort: null },
        latency: "standard",
        maxOutputTokens: null,
        cacheKey: "conversation-1",
      },
    },
    { emit: (event) => events.push(event) },
    {
      signal: new AbortController().signal,
      network: {
        fetch: () => Promise.reject(new Error("unexpected fetch")),
        stream: () =>
          Promise.resolve({
            status: 401,
            headers: {},
            lines: (async function* () {
              yield "unauthorized";
            })(),
          }),
      },
    },
  );

  assert(result.status === "auth-error", `expected auth-error, got ${result.status}`);
  assert(events.length === 0, "auth retry must happen before provider output");
});

Deno.test("Codex rejects invalid refresh grants without retrying the old token", async () => {
  const oldToken = jwt({ exp: Math.floor(Date.now() / 1000) - 1 });
  const resource = snapshot({
    accessToken: oldToken,
    refreshToken: "revoked-refresh-token",
    accountId: "account-1",
    displayName: "test",
    quota: null,
  });
  let calls = 0;
  const context = {
    signal: new AbortController().signal,
    network: {
      fetch: async () => {
        calls += 1;
        return { status: 400, headers: {}, body: JSON.stringify({ error: "invalid_grant" }) };
      },
      stream: () => Promise.reject(new Error("stream is not expected")),
    },
  } satisfies PluginContext;

  const patch = await prepareAccount(resource, null, context);
  assert(calls === 1, `expected one refresh request, got ${calls}`);
  assert(patch?.state?.status === "invalid", "revoked refresh token should require login");
});
