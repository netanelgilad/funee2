/// <reference path="../../../funee-lib/host/index.d.ts" />

import { fetch } from "host://http";
import { replacement } from "../../../funee-lib/replacements/index.ts";

export default [
  replacement(fetch, async (input: string | URL | Request) => {
    if (String(input) !== "https://upstream.example.test/message") {
      return Response.json({ message: "unexpected input" }, { status: 500 });
    }

    return Response.json({ message: "from in-memory fetch" });
  }),
];
