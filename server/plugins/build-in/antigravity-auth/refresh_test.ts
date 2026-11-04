import type { PluginContext } from "cursor-byok:plugin";
import type { ResourceSnapshot } from "cursor-byok:resource";
import plugin from "./main.ts";
import { RESOURCE_TYPE } from "./resources.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

const resource: ResourceSnapshot = {
  id: "account-1",
  type: RESOURCE_TYPE,
  key: "antigravity:test",
  privateData: { accessToken: "opaque", refreshToken: "refresh", projectId: "project" },
  state: { status: "ready" },
};
const refresh = plugin.resources!.find((item) => item.type === RESOURCE_TYPE)!.refresh!;

Deno.test("Google manual refresh forwards project and quota 401 to the host", async () => {
  for (const phase of ["loadCodeAssist", "fetchAvailableModels"]) {
    let calls = 0;
    const context: PluginContext = {
      signal: new AbortController().signal,
      network: {
        fetch: (url) => {
          calls++;
          return Promise.resolve({
            status: url.endsWith(phase) ? 401 : 200,
            headers: {},
            body: JSON.stringify({ cloudaicompanionProject: "project" }),
          });
        },
        stream: () => Promise.reject(new Error("unexpected stream")),
      },
    };
    const result = await refresh(resource, context);
    assert(result.status === "auth-error", `${phase}: expected auth-error, got ${result.status}`);
    assert(calls === (phase === "loadCodeAssist" ? 1 : 2), "401 must stop stale-token requests");
  }
});

Deno.test("Google manual refresh reports unavailable metadata instead of success", async () => {
  for (const phase of ["loadCodeAssist", "fetchAvailableModels"]) {
    for (const failure of ["http", "network", "malformed"]) {
      const context: PluginContext = {
        signal: new AbortController().signal,
        network: {
          fetch: (url) => {
            if (url.endsWith(phase)) {
              if (failure === "network") return Promise.reject(new Error("offline"));
              return Promise.resolve({
                status: failure === "http" ? 503 : 200,
                headers: {},
                body: failure === "malformed" ? "not json" : "{}",
              });
            }
            return Promise.resolve({
              status: 200,
              headers: {},
              body: JSON.stringify({ cloudaicompanionProject: "project", models: {} }),
            });
          },
          stream: () => Promise.reject(new Error("unexpected stream")),
        },
      };
      let error: unknown;
      try {
        await refresh(resource, context);
      } catch (caught) {
        error = caught;
      }
      assert(error instanceof Error, `${phase}/${failure}: failure must not produce a ready patch`);
    }
  }
});

Deno.test("Google manual refresh queries quota without exchanging a fresh token", async () => {
  const calls: string[] = [];
  const result = await refresh(resource, {
    signal: new AbortController().signal,
    network: {
      fetch: (url) => {
        calls.push(url);
        return Promise.resolve({
          status: 200,
          headers: {},
          body: JSON.stringify({
            cloudaicompanionProject: "project",
            models: { "gemini-test": { quotaInfo: { remainingFraction: 0.75 } } },
          }),
        });
      },
      stream: () => Promise.reject(new Error("unexpected stream")),
    },
  });
  assert(result.status === "completed", "quota refresh must succeed");
  assert(result.patch.state?.status === "ready", "successful quota refresh should be ready");
  assert(calls.length === 2 && calls.every((url) => !url.endsWith("/token")), "no token exchange");
});

Deno.test("Google manual refresh can use the next metadata endpoint after a temporary failure", async () => {
  for (const phase of ["loadCodeAssist", "fetchAvailableModels"]) {
    let attempts = 0;
    const result = await refresh(resource, {
      signal: new AbortController().signal,
      network: {
        fetch: (url) => {
          if (url.endsWith(phase) && ++attempts === 1) {
            return Promise.resolve({ status: 503, headers: {}, body: "{}" });
          }
          return Promise.resolve({
            status: 200,
            headers: {},
            body: JSON.stringify({ cloudaicompanionProject: "project", models: {} }),
          });
        },
        stream: () => Promise.reject(new Error("unexpected stream")),
      },
    });
    assert(attempts === 2, `${phase}: expected to try the next endpoint`);
    assert(result.status === "completed", "a successful alternate endpoint should recover");
  }
});
