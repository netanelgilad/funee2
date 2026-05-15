/// <reference path="./types.d.ts" />

import { serve, createJsonResponse } from "host://http/server";
import { env } from "host://process";

export default async () => {
  const port = Number(env("GATEWAY_PORT") ?? "18988");

  serve({ port }, () => {
    const body = { ok: true, service: "json-health-server" };
    return createJsonResponse(body, { status: 200 });
  });
};
