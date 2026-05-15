/// <reference path="../../funee-lib/host/process.d.ts" />

import { spawn } from "host://process";
import {
  scenario,
  runScenarios,
} from "../../funee-lib/validator/index.ts";
import { closure } from "../../funee-lib/macros/closure.ts";
import {
  assertThat,
  eventually,
  is,
  otherwise,
} from "../../funee-lib/assertions/index.ts";
import { buffer } from "../../funee-lib/streams/buffer.ts";
import { FUNEE_SUT_BIN } from "./_sut.ts";

const FUNEE = FUNEE_SUT_BIN;

const scenarios = [
  scenario({
    description: "gateway :: missing auth returns OpenAI-shaped 401",
    verify: closure(async () => {
      const port = 18987;

      await using gateway = spawn({
        cmd: [FUNEE, "tests/fixtures/gateway/ai-gateway-v0.ts"],
        env: {
          GATEWAY_PORT: String(port),
          GATEWAY_TOKEN: "test-gateway-token",
          MOCK_UPSTREAM_BASE_URL: "http://127.0.0.1:19998",
        },
      });
      const stdout = buffer(gateway.stdout);
      const stderr = buffer(gateway.stderr);

      const isServerHealthy = async () =>
        (await fetch(`http://127.0.0.1:${port}/healthz`)).status === 200;

      await assertThat(
        isServerHealthy,
        eventually(is(true)),
        otherwise(
          () =>
            "Gateway process output:\n\nstdout:\n" +
            stdout.text() +
            "\n\nstderr:\n" +
            stderr.text(),
        ),
      );

      const response = await fetch(
        `http://127.0.0.1:${port}/v1/chat/completions`,
        {
          method: "POST",
          headers: {
            "content-type": "application/json",
          },
          body: JSON.stringify({
            model: "fast",
            messages: [{ role: "user", content: "hello" }],
          }),
        },
      );

      const payload = (await response.json()) as {
        error?: {
          message?: string;
          type?: string;
          code?: string;
        };
      };

      await assertThat(response.status, is(401));
      await assertThat(payload.error?.type, is("invalid_request_error"));
      await assertThat(payload.error?.code, is("invalid_api_key"));
      await assertThat(
        payload.error?.message,
        is("Invalid or missing authentication token"),
      );
    }),
  }),
];

export default async () => {
  await runScenarios(scenarios);
};
