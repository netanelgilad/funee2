/// <reference path="../../funee-lib/host/process.d.ts" />

import { spawn } from "host://process";
import { scenario, runScenarios } from "../../funee-lib/validator/index.ts";
import { closure } from "../../funee-lib/macros/closure.ts";
import { assertThat, is } from "../../funee-lib/assertions/index.ts";
import { buffer } from "../../funee-lib/streams/buffer.ts";
import { FUNEE_SUT_BIN } from "./_sut.ts";

const FUNEE = FUNEE_SUT_BIN;

const scenarios = [
  scenario({
    description: "in-memory host :: configured fetch route replaces host fetch",
    verify: closure(async () => {
      await using result = spawn({
        cmd: [
          FUNEE,
          "--replacements",
          "tests/fixtures/in-memory-host/http-fetch-fixture.ts",
          "tests/fixtures/in-memory-host/fetch-entry.ts",
        ],
      });
      const stdout = buffer(result.stdout);

      const { status } = await result.completion();

      await assertThat(status, is(0));
      await assertThat(stdout.text(), is("upstream:from reusable in-memory host\n"));
    }),
  }),
];

export default async () => {
  await runScenarios(scenarios);
};
