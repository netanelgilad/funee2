/// <reference path="./types.d.ts" />

import { serve, createJsonResponse } from "host://http/server";
import { fetch } from "host://http";
import { env } from "host://process";

type GatewayErrorBody = {
  error: {
    message: string;
    type: string;
    param: string | null;
    code: string;
  };
};

function json(status: number, body: unknown): Response {
  return createJsonResponse(body, {
    status,
    headers: {
      "cache-control": "no-store",
    },
  });
}

function unauthorized(): Response {
  const body: GatewayErrorBody = {
    error: {
      message: "Invalid or missing authentication token",
      type: "invalid_request_error",
      param: null,
      code: "invalid_api_key",
    },
  };

  return json(401, body);
}

export default async () => {
  const port = Number(env("GATEWAY_PORT") ?? "18987");
  const gatewayToken = env("GATEWAY_TOKEN") ?? "test-gateway-token";
  const upstreamBaseUrl = env("MOCK_UPSTREAM_BASE_URL");

  serve({ port }, async (request) => {
    const url = new URL(request.url);

    if (request.method === "GET" && url.pathname === "/healthz") {
      return json(200, { ok: true, service: "funee-ai-gateway" });
    }

    if (request.method === "POST" && url.pathname === "/v1/chat/completions") {
      const auth = request.headers.get("authorization");
      const expected = `Bearer ${gatewayToken}`;
      if (!auth || auth !== expected) {
        return unauthorized();
      }

      const upstreamResponse = await fetch(
        `${upstreamBaseUrl}/v1/chat/completions`,
        {
          method: "POST",
          headers: {
            "content-type": request.headers.get("content-type") ??
              "application/json",
            authorization: auth,
          },
          body: await request.text(),
        },
      );
      const upstreamPayload = await upstreamResponse.json();

      return json(upstreamResponse.status, upstreamPayload);
    }

    return json(404, {
      error: {
        message: "Endpoint not supported by this gateway",
        type: "invalid_request_error",
        param: null,
        code: "endpoint_not_supported",
      },
    });
  });
};
