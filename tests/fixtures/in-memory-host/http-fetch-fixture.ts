/// <reference path="../../../funee-lib/host/index.d.ts" />

import { createInMemoryHost } from "../../../funee-lib/in-memory-host/index.ts";

export default createInMemoryHost({
  http: {
    servers: [
      {
        origin: "https://upstream.example.test",
        handler: async (request: Request) => {
          if (new URL(request.url).pathname === "/message") {
            return Response.json({ message: "from reusable in-memory host" });
          }

          return new Response("Not found", { status: 404 });
        },
      },
    ],
  },
});
