/// <reference path="../../../funee-lib/host/index.d.ts" />

import { log } from "host://console";
import { replacement } from "../../../funee-lib/replacements/index.ts";
import { formatMessage } from "./replacement-helper.ts";

export default [
  replacement(log, (message: string) => {
    console.log(formatMessage(message));
  }),
];
