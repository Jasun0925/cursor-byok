import type { JsonValue, PluginContext } from "cursor-byok:plugin";
import type { ResourcePatch, ResourceSnapshot } from "cursor-byok:resource";
import { accountData, tokenExpiresAt } from "./resources.ts";

export const CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann";
export const TOKEN_URL = "https://auth.openai.com/oauth/token";
const REFRESH_BUFFER_MS = 5 * 60 * 1000;

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function number(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/** The host holds the account lock and commits this patch BEFORE any inference begins. */
export async function prepareAccount(
  resource: ResourceSnapshot,
  rejectedResource: ResourceSnapshot | null,
  context: PluginContext,
): Promise<ResourcePatch | null> {
  const data = accountData(resource);
  const now = Date.now();
  const expiry = tokenExpiresAt(data);
  const rejected = rejectedResource !== null &&
    accountData(rejectedResource).accessToken === data.accessToken;
  const expired = () => expiry !== null && expiry <= Date.now();
  const stateInvalid = resource.state.status === "invalid";
  const failure = (message: string, terminal: boolean): ResourcePatch => {
    const next = { ...data };
    if (terminal) next.refreshError = message;
    else next.refreshRetryAtMs = Date.now() + 15_000;
    return {
      privateData: next as unknown as JsonValue,
      state: terminal
        ? { status: "invalid", message }
        : (expired() || rejected || stateInvalid)
        ? { status: "cooling", retryAtMs: next.refreshRetryAtMs!, message }
        : { status: "ready" },
    };
  };
  if (data.refreshError) return failure(data.refreshError, true);
  if (!rejected && !stateInvalid && (expiry === null || expiry > now + REFRESH_BUFFER_MS)) {
    return null;
  }
  if (data.refreshRetryAtMs && data.refreshRetryAtMs > now) {
    // Keep the original deadline; repeated calls must not postpone recovery indefinitely.
    return (expired() || rejected || stateInvalid)
      ? {
        state: {
          status: "cooling",
          retryAtMs: data.refreshRetryAtMs,
          message: "ChatGPT token refresh temporarily unavailable; retry shortly",
        },
      }
      : null;
  }
  if (!data.refreshToken) return failure("ChatGPT refresh token is missing; sign in again", true);
  let response;
  try {
    response = await context.network.fetch(TOKEN_URL, {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/x-www-form-urlencoded",
      },
      body: new URLSearchParams({
        grant_type: "refresh_token",
        client_id: CLIENT_ID,
        refresh_token: data.refreshToken,
      }).toString(),
    });
  } catch {
    // A lost response may have rotated the token. Do not replay it in this call.
    return failure("ChatGPT token refresh network error; retry shortly", false);
  }
  let body: Record<string, unknown> = {};
  try {
    body = object(JSON.parse(response.body)) ?? {};
  } catch { /* classified below */ }
  if (response.status >= 200 && response.status < 300) {
    const accessToken = text(body.access_token);
    const refreshToken = text(body.refresh_token) ?? data.refreshToken;
    // Even a malformed successful response must not discard a rotated refresh token.
    if (!accessToken) {
      data.refreshToken = refreshToken;
      return failure("ChatGPT token refresh returned no access token; retry shortly", false);
    }
    const seconds = number(body.expires_in);
    const jwtExpiry = tokenExpiresAt({ ...data, accessToken, expiresAtMs: undefined });
    const expiresAtMs = seconds !== null && seconds > 0
      ? Date.now() + seconds * 1000
      : jwtExpiry !== null
      ? jwtExpiry
      : undefined;
    const { refreshError: _error, refreshRetryAtMs: _retry, expiresAtMs: _expiry, ...previous } =
      data;
    return {
      privateData: {
        ...previous,
        accessToken,
        refreshToken,
        ...(expiresAtMs ? { expiresAtMs } : {}),
      } as unknown as JsonValue,
      state: { status: "ready" },
    };
  }
  const code = text(body.error);
  if (code === "invalid_grant" || code === "invalid_client") {
    return failure(`ChatGPT refresh authorization rejected (${code}); sign in again`, true);
  }
  return failure(
    `ChatGPT token refresh temporarily failed (HTTP ${response.status}); retry shortly`,
    false,
  );
}
