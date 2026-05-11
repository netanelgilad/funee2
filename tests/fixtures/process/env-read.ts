/// <reference path="../../../funee-lib/host/index.d.ts" />

import { env } from "host://process";
import { log } from "host://console";

export default () => {
  const value = env("TEST_ENV_VALUE");
  log(`env-value:${value ?? "undefined"}`);
};
