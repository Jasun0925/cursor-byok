import { defineProviderPlugin } from "cursor-byok:plugin";
import type { PluginContext } from "cursor-byok:plugin";
import { prepareAccount, refreshAccount } from "../../../plugins/build-in/grok-auth/resources.ts";
import { prepareAccount as prepareGoogle } from "../../../plugins/build-in/antigravity-auth/credentials.ts";
import { refreshAccount as refreshGoogle } from "../../../plugins/build-in/antigravity-auth/resources.ts";

type TestData = { token: string; delayMs?: number; rejectedDelayMs?: number };

// Synthetic worker state is only observed by the host integration tests.
let exchanges = 0;
const invocations: unknown[] = [];
const grokCalls: string[] = [];
const googleCalls: string[] = [];
const googleContext: PluginContext = {
  signal: new AbortController().signal,
  network: {
    fetch: async (url, init) => {
      const token = init?.headers?.authorization ?? "";
      if (url.endsWith("/token")) {
        googleCalls.push("token");
        const mode = new URLSearchParams(init?.body).get("refresh_token")!.replace("refresh-google-", "");
        if (mode === "revoked") {
          return { status: 400, headers: {}, body: JSON.stringify({ error: "invalid_grant" }) };
        }
        return { status: 200, headers: {}, body: JSON.stringify({
          access_token: `opaque-google-${mode}-rotated`, refresh_token: `rotated-google-${mode}`, expires_in: 3600,
        }) };
      }
      const phase = url.endsWith("loadCodeAssist") ? "project" : "quota";
      googleCalls.push(`${phase}:${token}`);
      const rotated = token.endsWith("-rotated");
      const status = phase === "project"
        ? (token.includes("-project") && !rotated ? 401 : 200)
        : rotated ? (token.includes("-reject") ? 401 : token.includes("-unavailable") ? 503 : 200) : 401;
      return { status, headers: {}, body: JSON.stringify({
        cloudaicompanionProject: "synthetic-project",
        models: { "gemini-test": { quotaInfo: { remainingFraction: 0.75 } } },
      }) };
    },
    stream: () => { throw new Error("unexpected inference network call"); },
  },
};
const grokContext: PluginContext = {
  signal: new AbortController().signal,
  network: {
    fetch: async (url, init) => {
      const token = init?.headers?.authorization ?? "";
      grokCalls.push(url.includes("oauth2/token") ? "token" : token);
      if (url.includes("oauth2/token")) {
        const mode = new URLSearchParams(init?.body).get("refresh_token")!.replace("refresh-", "");
        return { status: 200, headers: {}, body: JSON.stringify({
          access_token: `opaque-${mode}-rotated`, refresh_token: `rotated-${mode}`, expires_in: 3600,
        }) };
      }
      const status = token.includes("-rotated")
        ? (token.includes("reject") ? 401 : token.includes("unavailable") ? 503 : 200)
        : 401;
      return { status, headers: {}, body: JSON.stringify({ creditUsagePercent: 25 }) };
    },
    stream: () => { throw new Error("unexpected inference network call"); },
  },
};

export default defineProviderPlugin({
  providers: [{
    id: "fake", displayName: "fake", providerType: "test", resourceType: "account",
    invoke: async (input, output) => {
      invocations.push(structuredClone(input));
      const scenario = input.request.instructions;
      if (scenario === "after-output") {
        output.emit({ type: "text-delta", text: "partial" });
      }
      if (scenario === "always-reject" || scenario === "after-output" ||
          (scenario === "retry" && invocations.length === 1)) {
        return { status: "auth-error", message: "synthetic 401" };
      }
      output.emit({ type: "text-delta", text: (input.resource!.privateData as TestData).token });
      output.emit({ type: "done", reason: "stop" });
      if (scenario === "late-quota-patch") {
        await new Promise(resolve => setTimeout(resolve, 300));
        return { status: "completed", patch: {
          privateData: { ...(input.resource!.privateData as TestData), quota: 75 },
        } };
      }
      return { status: "completed" };
    },
  }],
  resources: [{
    type: "account", displayName: "account",
    present: () => ({ displayName: JSON.stringify({ exchanges, invocations, grokCalls, googleCalls }) }),
    prepare: async (resource, rejected) => {
      const data = resource.privateData as TestData;
      if (data.token.startsWith("fresh-") && !rejected) return null;
      const generation = ++exchanges;
      await new Promise(resolve => setTimeout(resolve,
        rejected ? (data.rejectedDelayMs ?? 150) : (data.delayMs ?? 150)));
      return {
        privateData: { ...data, token: `fresh-${generation}`, refreshToken: `rotated-${generation}` },
        state: { status: "ready" },
      };
    },
  }, {
    type: "grok-account", displayName: "Synthetic Grok account",
    present: () => ({ displayName: "test" }),
    prepare: (resource, rejected) => prepareAccount(resource, rejected, grokContext),
    refresh: (resource) => refreshAccount(resource, grokContext),
  }, {
    type: "antigravity-account", displayName: "Synthetic Google account",
    present: () => ({ displayName: "test" }),
    prepare: (resource, rejected) => prepareGoogle(resource, rejected, googleContext),
    refresh: (resource) => refreshGoogle(resource, googleContext),
  }],
});
