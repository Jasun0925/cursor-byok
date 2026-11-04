import type { JsonValue, PluginContext } from "cursor-byok:plugin";
import type { ResourceSnapshot } from "cursor-byok:resource";
import { prepareAccount } from "./credentials.ts";
import { CLIENT_ID, CLIENT_SECRET, GOOGLE_TOKEN_URL } from "./google_oauth.ts";
import { RESOURCE_TYPE } from "./resources.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function snapshot(
  privateData: JsonValue,
  state: ResourceSnapshot["state"] = { status: "ready" },
): ResourceSnapshot {
  return { id: "account-1", type: RESOURCE_TYPE, key: "antigravity:account-1", privateData, state };
}

Deno.test("Google refresh persists a rotated token and explicit expiry", async () => {
  let calls = 0;
  const context = {
    signal: new AbortController().signal,
    network: {
      fetch: async (url: string, init?: { headers?: Record<string, string>; body?: string }) => {
        calls += 1;
        assert(url === GOOGLE_TOKEN_URL, `unexpected token URL: ${url}`);
        assert(
          init?.headers?.["content-type"] === "application/x-www-form-urlencoded",
          "Google token endpoint requires form encoding",
        );
        const body = new URLSearchParams(init?.body);
        assert(body.get("client_id") === CLIENT_ID, "wrong Google client ID");
        assert(body.get("client_secret") === CLIENT_SECRET, "wrong Google client secret");
        assert(body.get("grant_type") === "refresh_token", "wrong grant type");
        assert(body.get("refresh_token") === "refresh-before", "wrong refresh token");
        return {
          status: 200,
          headers: {},
          body: JSON.stringify({
            access_token: "google-access-after",
            refresh_token: "refresh-after",
            expires_in: 3600,
          }),
        };
      },
      stream: () => Promise.reject(new Error("stream is not expected")),
    },
  } satisfies PluginContext;
  const patch = await prepareAccount(
    snapshot({
      accessToken: "google-access-before",
      refreshToken: "refresh-before",
      displayName: "test",
      projectId: "project",
      quota: null,
      expiresAtMs: Date.now() - 1,
    }),
    null,
    context,
  );
  assert(calls === 1, `expected one refresh request, got ${calls}`);
  const privateData = patch?.privateData as Record<string, JsonValue>;
  assert(patch?.state?.status === "ready", "refreshed account should be ready");
  assert(privateData.accessToken === "google-access-after", "new access token missing");
  assert(privateData.refreshToken === "refresh-after", "rotated refresh token missing");
  assert(
    typeof privateData.expiresAtMs === "number" && privateData.expiresAtMs > Date.now(),
    "expiry missing",
  );
});

Deno.test("Google invalid_grant requires a new login and is not retried", async () => {
  let calls = 0;
  const context = {
    signal: new AbortController().signal,
    network: {
      fetch: () => {
        calls += 1;
        return Promise.resolve({
          status: 400,
          headers: {},
          body: JSON.stringify({ error: "invalid_grant" }),
        });
      },
      stream: () => Promise.reject(new Error("stream is not expected")),
    },
  } satisfies PluginContext;
  const patch = await prepareAccount(
    snapshot({
      accessToken: "google-access-before",
      refreshToken: "revoked-refresh",
      displayName: "test",
      quota: null,
      expiresAtMs: Date.now() - 1,
    }),
    null,
    context,
  );
  assert(patch?.state?.status === "invalid", "revoked Google token should require login");
  const saved = snapshot(patch.privateData as JsonValue, patch.state);
  for (const rejected of [null, saved]) {
    const rejectedAgain = await prepareAccount(saved, rejected, context);
    assert(rejectedAgain?.state?.status === "invalid", "revoked token state should remain invalid");
  }
  assert(calls === 1, "revoked refresh token must not be sent repeatedly");
});

Deno.test("Google can recover an invalid access token unless its refresh grant was revoked", async () => {
  for (const expiresAtMs of [Date.now() - 1, null]) {
    let calls = 0;
    const context = {
      signal: new AbortController().signal,
      network: {
        fetch: () => {
          calls++;
          return Promise.resolve({
            status: 200,
            headers: {},
            body: JSON.stringify({ access_token: "recovered", expires_in: 3600 }),
          });
        },
        stream: () => Promise.reject(new Error("unexpected stream")),
      },
    } satisfies PluginContext;
    const patch = await prepareAccount(
      snapshot(
        { accessToken: "rejected", refreshToken: "still-valid", expiresAtMs },
        { status: "invalid", message: "Access token expired" },
      ),
      null,
      context,
    );
    assert(calls === 1, `expected recovery exchange, got ${calls}`);
    assert(patch?.state?.status === "ready", "recoverable account must become ready");
    assert(
      (patch.privateData as Record<string, JsonValue>).accessToken === "recovered",
      "recovered token must be persisted",
    );
  }
});

Deno.test("Google malformed refresh response cools down and preserves rotation", async () => {
  const context = {
    signal: new AbortController().signal,
    network: {
      fetch: () =>
        Promise.resolve({
          status: 200,
          headers: {},
          body: JSON.stringify({ refresh_token: "refresh-after" }),
        }),
      stream: () => Promise.reject(new Error("stream is not expected")),
    },
  } satisfies PluginContext;
  const patch = await prepareAccount(
    snapshot({
      accessToken: "google-access-before",
      refreshToken: "refresh-before",
      displayName: "test",
      quota: null,
      expiresAtMs: Date.now() - 1,
    }),
    null,
    context,
  );
  const privateData = patch?.privateData as Record<string, JsonValue>;
  assert(patch?.state?.status === "cooling", "malformed response should defer retry");
  assert(privateData.refreshToken === "refresh-after", "returned rotation must be persisted");
  assert(typeof privateData.refreshRetryAtMs === "number", "retry deadline must be persisted");
});

Deno.test("Google opaque access tokens receive a fallback expiry", async () => {
  const context = {
    signal: new AbortController().signal,
    network: {
      fetch: () =>
        Promise.resolve({
          status: 200,
          headers: {},
          body: JSON.stringify({ access_token: "opaque-access-after" }),
        }),
      stream: () => Promise.reject(new Error("stream is not expected")),
    },
  } satisfies PluginContext;
  const patch = await prepareAccount(
    snapshot({
      accessToken: "opaque-access-before",
      refreshToken: "refresh-before",
      displayName: "test",
      quota: null,
      expiresAtMs: Date.now() - 1,
    }),
    null,
    context,
  );
  const privateData = patch?.privateData as Record<string, JsonValue>;
  assert(patch?.state?.status === "ready", "successful refresh should be ready");
  assert(privateData.accessToken === "opaque-access-after", "new access token missing");
  assert(
    typeof privateData.expiresAtMs === "number" && privateData.expiresAtMs > Date.now(),
    "opaque access token needs a fallback expiry",
  );
});

Deno.test("Google refresh cooldown survives network errors and delayed rejected requests", async () => {
  for (const mode of ["network", "http", "malformed"] as const) {
    let calls = 0;
    const resource = snapshot({
      accessToken: "expired",
      refreshToken: "refresh",
      expiresAtMs: Date.now() - 1,
    });
    const context = {
      signal: new AbortController().signal,
      network: {
        fetch: () => {
          calls++;
          if (mode === "network") return Promise.reject(new Error("connection lost"));
          return Promise.resolve({ status: mode === "http" ? 503 : 200, headers: {}, body: "{}" });
        },
        stream: () => Promise.reject(new Error("unexpected stream")),
      },
    } satisfies PluginContext;
    const first = await prepareAccount(resource, null, context);
    assert(first?.state?.status === "cooling", "refresh failure must enter cooldown");
    const saved = snapshot(first.privateData ?? resource.privateData, first.state);
    for (const rejected of [null, resource]) {
      const second = await prepareAccount(saved, rejected, context);
      assert(second?.state?.status === "cooling", "later requests must respect cooldown");
      assert(second.state.retryAtMs === first.state.retryAtMs, "retry deadline must not slide");
    }
    assert(calls === 1, `${mode}: expected one refresh during cooldown, got ${calls}`);
  }
});
