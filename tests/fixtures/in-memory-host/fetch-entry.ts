/// <reference path="../../../funee-lib/host/index.d.ts" />

import { log } from "host://console";
import { fetch } from "host://http";

export default async () => {
  const response = await fetch("https://upstream.example.test/message");
  const payload = (await response.json()) as { message: string };

  log(`upstream:${payload.message}`);
};
