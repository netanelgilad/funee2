import { fetch } from "host://http";
import { replacement } from "../replacements/index.ts";

export type InMemoryHttpServer = {
  origin: string;
  handler: (request: Request) => Response | Promise<Response>;
};

export type InMemoryHostConfig = {
  http?: {
    servers?: Array<InMemoryHttpServer>;
  };
};

export const createInMemoryHost = (config: InMemoryHostConfig) => {
  const servers = config.http?.servers ?? [];

  return [
    replacement(fetch, async (input: string | URL | Request, init?: RequestInit) => {
      const request = input instanceof Request
        ? input
        : new Request(input, init);
      const requestOrigin = new URL(request.url).origin;
      const server = servers.find((server) => server.origin === requestOrigin);

      if (!server) {
        return new Response(`No in-memory HTTP server for ${requestOrigin}`, {
          status: 502,
        });
      }

      return await server.handler(request);
    }),
  ];
};
