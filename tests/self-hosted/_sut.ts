/// <reference path="../../funee-lib/host/process.d.ts" />

import { spawn, type CommandOutput, type Process } from "host://process";
import type { Closure } from "../../funee-lib/core.ts";
import { composeAsyncDisposables } from "../../funee-lib/disposables/async.ts";
import {
  defaultExportFixture,
  type DefaultExportFixture,
} from "../../funee-lib/fixtures/default-export.ts";

export const FUNEE_SUT_BIN = "./target/sut/funee";

export const runFuneeSUT = (args: string[]): Promise<CommandOutput> =>
  spawn(FUNEE_SUT_BIN, args);

type SUTModule = string | Closure<unknown>;

export type SpawnFuneeSUTOptions = {
  entrypoint: SUTModule;
  inMemoryHost?: SUTModule;
  env?: Record<string, string>;
};

const modulePathFor = async (
  module: SUTModule,
): Promise<{ path: string; fixture?: DefaultExportFixture }> => {
  if (typeof module === "string") {
    return { path: module };
  }

  const fixture = await defaultExportFixture(module);
  return { path: fixture.path, fixture };
};

export const spawnFuneeSUT = async ({
  entrypoint,
  inMemoryHost,
  env,
}: SpawnFuneeSUTOptions): Promise<Process> => {
  const entrypointModule = await modulePathFor(entrypoint);
  const inMemoryHostModule = inMemoryHost
    ? await modulePathFor(inMemoryHost)
    : undefined;
  const cmd = inMemoryHostModule
    ? [
      FUNEE_SUT_BIN,
      "--replacements",
      inMemoryHostModule.path,
      entrypointModule.path,
    ]
    : [FUNEE_SUT_BIN, entrypointModule.path];
  const process = spawn({ cmd, env });

  return composeAsyncDisposables(process, [
    inMemoryHostModule?.fixture,
    entrypointModule.fixture,
  ]);
};
