/// <reference path="../../../funee-lib/host/index.d.ts" />

import { createInMemoryHost } from "../../../funee-lib/in-memory-host/index.ts";

export default createInMemoryHost({
  http: {
    servers: [
      {
        origin: "https://upstream.example.test",
        handler: async (request: Request) => {
          const url = new URL(request.url);

          if (
            request.method === "POST" &&
            url.pathname === "/v1/chat/completions"
          ) {
            return Response.json({
              id: "chatcmpl_in_memory_upstream",
              object: "chat.completion",
              created: 0,
              model: "fast",
              choices: [
                {
                  index: 0,
                  message: {
                    role: "assistant",
                    content: "from in-memory upstream",
                  },
                  finish_reason: "stop",
                },
              ],
            });
          }

          return Response.json(
            {
              error: {
                message: "Unexpected upstream request",
                type: "invalid_request_error",
                param: null,
                code: "unexpected_upstream_request",
              },
            },
            { status: 404 },
          );
        },
      },
    ],
  },
});
