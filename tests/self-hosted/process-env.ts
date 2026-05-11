/// <reference path="../../funee-lib/host/index.d.ts" />

import { spawn } from "host://process";
import { scenario, runScenarios } from "../../funee-lib/validator/index.ts";
import { closure } from "../../funee-lib/macros/closure.ts";
import { assertThat, is } from "../../funee-lib/assertions/index.ts";
import { buffer } from "../../funee-lib/streams/buffer.ts";
import { FUNEE_SUT_BIN } from "./_sut.ts";

const FUNEE = FUNEE_SUT_BIN;

const scenarios = [
  scenario({
    description: "process env :: env(name) reads injected environment variable",
    verify: closure(async () => {
      await using result = spawn({
        cmd: [FUNEE, "tests/fixtures/process/env-read.ts"],
        env: {
          TEST_ENV_VALUE: "from-self-hosted-test",
        },
      });

      const stdout = buffer(result.stdout);

      const { status } = await result.completion();

      await assertThat(status, is(0));
      await assertThat(stdout.text(), is("env-value:from-self-hosted-test\n"));
    }),
  }),
];

export default async () => {
  await runScenarios(scenarios);
};
