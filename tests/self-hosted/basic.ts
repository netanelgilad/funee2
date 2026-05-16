import {
  log,
  scenario,
  runScenarios,
  closure,
  assertThat,
  is,
  contains,
  greaterThan,
} from "funee";
import { runFuneeSUT } from "./_sut.ts";

const scenarios = [
  // ==================== BASIC EXECUTION ====================

  scenario({
    description: "basic :: runs hello.ts",
    verify: closure(async () => {
        const result = await runFuneeSUT(["tests/fixtures/hello.ts"]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("hello from funee"));
      }),
  }),

  scenario({
    description: "basic :: runs default export expressions",
    verify: closure(async () => {
        const result = await runFuneeSUT(["tests/fixtures/default-expr.ts"]);
        await assertThat(result.status.code, is(0));
        await assertThat(
          result.stdoutText(),
          contains("default export expression works")
        );
      }),
  }),

  scenario({
    description: "basic :: supports multiple host functions",
    verify: closure(async () => {
        const result = await runFuneeSUT(["tests/fixtures/multi-host.ts"]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("log works"));
        await assertThat(result.stdoutText(), contains("[DEBUG] debug works"));
      }),
  }),

  scenario({
    description: "basic :: supports async functions",
    verify: closure(async () => {
        const result = await runFuneeSUT(["tests/fixtures/async.ts"]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("async start"));
        await assertThat(result.stdoutText(), contains("async helper called"));
        await assertThat(result.stdoutText(), contains("async end"));
      }),
  }),

  // ==================== TREE SHAKING ====================

  scenario({
    description: "tree shaking :: only includes referenced declarations",
    verify: closure(async () => {
        const result = await runFuneeSUT(["tests/fixtures/treeshake/entry.ts"]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("used function"));
        await assertThat(result.stdoutText(), contains("tree shaking works"));
        // Should NOT contain unused functions' output
        const stdout = result.stdoutText();
        await assertThat(
          stdout.includes("unused function - should NOT appear"),
          is(false)
        );
        await assertThat(
          stdout.includes("also unused - should NOT appear"),
          is(false)
        );
      }),
  }),

  scenario({
    description: "tree shaking :: emitted code does not contain unused declarations",
    verify: closure(async () => {
        const result = await runFuneeSUT([
          "--emit",
          "tests/fixtures/treeshake/entry.ts",
        ]);
        await assertThat(result.status.code, is(0));
        // The emitted JS should contain the used function
        await assertThat(result.stdoutText(), contains("used"));
        // But should NOT contain the unused functions
        // Note: declarations are renamed to declaration_N, so we check for the
        // string literals from the unused functions
        const stdout = result.stdoutText();
        await assertThat(
          stdout.includes("unused function - should NOT appear"),
          is(false)
        );
        await assertThat(
          stdout.includes("also unused - should NOT appear"),
          is(false)
        );
      }),
  }),

  // ==================== GLOBALS ====================

  scenario({
    description: "globals :: supports JavaScript built-in globals",
    verify: closure(async () => {
        const result = await runFuneeSUT(["tests/fixtures/globals.ts"]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("Promise.resolve: 42"));
        await assertThat(result.stdoutText(), contains("Promise.all: a,b,c"));
        await assertThat(result.stdoutText(), contains("Array.map: 2,4,6"));
        await assertThat(result.stdoutText(), contains("Object.keys: a,b"));
        await assertThat(
          result.stdoutText(),
          contains('JSON.stringify: {"test":true}')
        );
        await assertThat(result.stdoutText(), contains("Math.max: 5"));
        await assertThat(result.stdoutText(), contains("globals test complete"));
      }),
  }),

  scenario({
    description: "globals :: tree-shakes but preserves global references in emitted code",
    verify: closure(async () => {
        const result = await runFuneeSUT(["--emit", "tests/fixtures/globals.ts"]);
        await assertThat(result.status.code, is(0));
        // Globals should be referenced directly, not as imports
        await assertThat(result.stdoutText(), contains("Promise"));
        await assertThat(result.stdoutText(), contains("Object"));
        await assertThat(result.stdoutText(), contains("JSON"));
      }),
  }),

  // ==================== ERROR HANDLING ====================

  scenario({
    description: "error handling :: reports missing import errors",
    verify: closure(async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/errors/missing-import.ts",
        ]);
        // Should exit with non-zero code
        await assertThat(result.status.code, greaterThan(0));
        // Should mention what couldn't be found
        await assertThat(result.stderrText(), contains("doesNotExist"));
      }),
  }),

  scenario({
    description: "error handling :: reports parse errors",
    verify: closure(async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/errors/syntax-error.ts",
        ]);
        // Should exit with non-zero code
        await assertThat(result.status.code, greaterThan(0));
        // Should indicate a parse/syntax error occurred
        const stderr = result.stderrText().toLowerCase();
        const hasErrorInfo =
          stderr.includes("parse") ||
          stderr.includes("error") ||
          stderr.includes("expected");
        await assertThat(hasErrorInfo, is(true));
      }),
  }),
];

export default async () => {
  await runScenarios(scenarios, { logger: log });
};
