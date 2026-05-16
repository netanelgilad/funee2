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
    description:
      "replacements :: CLI loads replacements before executing entrypoint",
    verify: closure(async () => {
      await using result = spawn({
        cmd: [
          FUNEE,
          "--replacements",
          "tests/fixtures/replacements/log-replacements.ts",
          "tests/fixtures/replacements/log-entry.ts",
        ],
      });
      const stdout = buffer(result.stdout);

      const { status } = await result.completion();

      await assertThat(status, is(0));
      await assertThat(stdout.text(), is("memory:hello from entry\n"));
    }),
  }),
  scenario({
    description: "replacements :: host fetch can be replaced in spawned runtime",
    verify: closure(async () => {
      await using result = spawn({
        cmd: [
          FUNEE,
          "--replacements",
          "tests/fixtures/replacements/fetch-replacements.ts",
          "tests/fixtures/replacements/fetch-entry.ts",
        ],
      });
      const stdout = buffer(result.stdout);

      const { status } = await result.completion();

      await assertThat(status, is(0));
      await assertThat(stdout.text(), is("upstream:from in-memory fetch\n"));
    }),
  }),
  scenario({
    description: "replacements :: implementation dependencies are included",
    verify: closure(async () => {
      await using result = spawn({
        cmd: [
          FUNEE,
          "--replacements",
          "tests/fixtures/replacements/dependent-log-replacements.ts",
          "tests/fixtures/replacements/log-entry.ts",
        ],
      });
      const stdout = buffer(result.stdout);

      const { status } = await result.completion();

      await assertThat(status, is(0));
      await assertThat(stdout.text(), is("memory:helper says hello from entry\n"));
    }),
  }),
];

export default async () => {
  await runScenarios(scenarios);
};
