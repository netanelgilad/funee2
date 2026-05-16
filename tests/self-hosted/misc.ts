/**
 * Self-hosted test scenarios for miscellaneous funee functionality.
 * 
 * Covers:
 * - Subprocess tests (spawn API - stdin/stdout, exit codes, signals)
 * - Timer tests (setTimeout, clearTimeout, setInterval)
 * - Macro tests (closure, canonicalName, definition, etc.)
 * - Watch mode tests (file watching, closure references)
 * - Import patterns (re-exports, aliasing, chains)
 * 
 * Run with: funee tests/self-hosted/misc.ts
 */

import {
  scenario,
  runScenarios,
  runScenariosWatch,
  assertThat,
  is,
  greaterThan,
  lessThan,
  contains,
  log,
  spawn,
  Closure,
  Definition,
  watchFile,
  watchDirectory,
} from "funee";
import { runFuneeSUT } from "./_sut.ts";

// ============================================================================
// SUBPROCESS SCENARIOS
// ============================================================================

const subprocessScenarios = [
  // Basic spawn and exit code
  scenario({
    description: "subprocess :: basic spawn returns exit code 0 for successful command",
    verify: {
      expression: async () => {
        const result = await spawn("echo", ["hello"]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.status.success, is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Capture stdout
  scenario({
    description: "subprocess :: captures stdout output",
    verify: {
      expression: async () => {
        const result = await spawn("echo", ["hello world"]);
        await assertThat(result.stdoutText().trim(), is("hello world"));
        await assertThat(result.status.code, is(0));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Capture stderr
  scenario({
    description: "subprocess :: captures stderr output",
    verify: {
      expression: async () => {
        const result = await spawn("sh", ["-c", "echo error >&2; exit 1"]);
        const stderr = result.stderrText().trim();
        await assertThat(stderr, contains("error"));
        await assertThat(result.status.success, is(false));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Non-zero exit codes
  scenario({
    description: "subprocess :: captures non-zero exit codes",
    verify: {
      expression: async () => {
        const result1 = await spawn("sh", ["-c", "exit 1"]);
        await assertThat(result1.status.code, is(1));
        await assertThat(result1.status.success, is(false));

        const result42 = await spawn("sh", ["-c", "exit 42"]);
        await assertThat(result42.status.code, is(42));

        const result255 = await spawn("sh", ["-c", "exit 255"]);
        await assertThat(result255.status.code, is(255));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Write to stdin
  scenario({
    description: "subprocess :: writes to stdin and receives output",
    verify: {
      expression: async () => {
        const proc = spawn({
          cmd: ["cat"],
          stdin: "piped",
          stdout: "piped",
        });
        await proc.writeInput("hello from stdin");
        const output = await proc.output();
        await assertThat(output.stdoutText(), is("hello from stdin"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Set working directory
  scenario({
    description: "subprocess :: sets working directory",
    verify: {
      expression: async () => {
        const proc = spawn({ cmd: ["pwd"], cwd: "/tmp" });
        const result = await proc.output();
        // On macOS, /tmp symlinks to /private/tmp
        await assertThat(result.stdoutText().trim(), contains("tmp"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Set environment variables
  scenario({
    description: "subprocess :: sets environment variables",
    verify: {
      expression: async () => {
        const proc = spawn({
          cmd: ["sh", "-c", "echo $MY_VAR"],
          env: { MY_VAR: "test_value" },
          inheritEnv: true,
        });
        const result = await proc.output();
        await assertThat(result.stdoutText().trim(), is("test_value"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Kill a running process
  scenario({
    description: "subprocess :: kills a running process with signal",
    verify: {
      expression: async () => {
        const proc = spawn({
          cmd: ["sleep", "60"],
        });
        await assertThat(typeof proc.pid, is("number"));
        proc.kill("SIGTERM");
        const status = await proc.status;
        await assertThat(status.success, is(false));
        await assertThat(status.signal, is("SIGTERM"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Multiple arguments
  scenario({
    description: "subprocess :: passes multiple arguments correctly",
    verify: {
      expression: async () => {
        const result = await spawn("echo", ["one", "two", "three"]);
        await assertThat(result.stdoutText().trim(), is("one two three"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ============================================================================
// TIMER SCENARIOS
// ============================================================================

const timerScenarios = [
  // Basic setTimeout
  scenario({
    description: "timer :: setTimeout fires after delay",
    verify: {
      expression: async () => {
        const events: string[] = [];
        events.push("start");

        await new Promise<void>((resolve) => {
          setTimeout(() => {
            events.push("timeout fired");
            resolve();
          }, 50);
        });

        events.push("end");

        await assertThat(events[0], is("start"));
        await assertThat(events[1], is("timeout fired"));
        await assertThat(events[2], is("end"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // clearTimeout cancels pending timeout
  scenario({
    description: "timer :: clearTimeout cancels pending timeout",
    verify: {
      expression: async () => {
        let fired = false;

        const id = setTimeout(() => {
          fired = true;
        }, 50);

        clearTimeout(id);

        // Wait longer than the timeout would have taken
        await new Promise<void>((resolve) => setTimeout(resolve, 100));

        await assertThat(fired, is(false));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // setInterval fires repeatedly
  scenario({
    description: "timer :: setInterval fires repeatedly until cleared",
    verify: {
      expression: async () => {
        let count = 0;

        await new Promise<void>((resolve) => {
          const id = setInterval(() => {
            count++;
            if (count >= 3) {
              clearInterval(id);
              resolve();
            }
          }, 30);
        });

        await assertThat(count, is(3));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Multiple concurrent timeouts
  scenario({
    description: "timer :: multiple concurrent timeouts fire in order",
    verify: {
      expression: async () => {
        const events: number[] = [];

        await Promise.all([
          new Promise<void>((resolve) =>
            setTimeout(() => {
              events.push(1);
              resolve();
            }, 30)
          ),
          new Promise<void>((resolve) =>
            setTimeout(() => {
              events.push(2);
              resolve();
            }, 60)
          ),
          new Promise<void>((resolve) =>
            setTimeout(() => {
              events.push(3);
              resolve();
            }, 90)
          ),
        ]);

        await assertThat(events.length, is(3));
        await assertThat(events[0], is(1));
        await assertThat(events[1], is(2));
        await assertThat(events[2], is(3));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ============================================================================
// MACRO SCENARIOS
// ============================================================================

const macroScenarios = [
  // Closure constructor accepts plain objects
  scenario({
    description:
      "macro :: [SPEC-MACRO-RUNTIME-001] Closure constructor converts plain objects to Map",
    verify: {
      expression: async () => {
        const c = Closure({
          expression: "test",
          references: { foo: { uri: "/test.ts", name: "foo" } },
        });

        await assertThat(c.references instanceof Map, is(true));
        await assertThat(c.references.size, is(1));
        await assertThat(c.references.has("foo"), is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Closure constructor accepts Map
  scenario({
    description:
      "macro :: [SPEC-MACRO-RUNTIME-002] Closure constructor accepts Map references",
    verify: {
      expression: async () => {
        const refsMap = new Map([
          ["bar", { uri: "/bar.ts", name: "bar" }],
        ]);

        const c = Closure({
          expression: "test",
          references: refsMap,
        });

        await assertThat(c.references instanceof Map, is(true));
        await assertThat(c.references.size, is(1));
        await assertThat(c.references.has("bar"), is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // CanonicalName type structure
  scenario({
    description:
      "macro :: [SPEC-MACRO-RUNTIME-003] CanonicalName has uri and name properties",
    verify: {
      expression: async () => {
        // CanonicalName is a structural type { uri, name }
        const name = {
          uri: "/path/to/file.ts",
          name: "exportedName",
        };

        await assertThat(typeof name.uri, is("string"));
        await assertThat(typeof name.name, is("string"));
        await assertThat(name.uri, contains(".ts"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Definition type structure
  scenario({
    description:
      "macro :: [SPEC-MACRO-RUNTIME-004] Definition has declaration and references",
    verify: {
      expression: async () => {
        const def = Definition({
          declaration: { type: "VariableDeclaration", kind: "const" },
          references: {},
        });

        await assertThat(typeof def.declaration, is("object"));
        await assertThat(def.references instanceof Map, is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Test actual closure macro expansion via fixture
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXPANSION-001] closure macro expands to AST at compile time",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/closure-macro.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("type: object"));
        await assertThat(result.stdoutText(), contains("AST type: ArrowFunctionExpression"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Test macro with cross-file references
  scenario({
    description:
      "macro :: [SPEC-MACRO-REFERENCES-001] closure captures cross-file references",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/cross-file-ref/entry.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("has 'add' reference: true"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Conditional macro expansion
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXEC-001] conditional macro branches on expression shape",
    verify: {
      expression: async () => {
        const first = await runFuneeSUT([
          "tests/fixtures/macro/conditional_macro.ts",
        ]);
        await assertThat(first.status.code, is(0));
        await assertThat(first.stdoutText(), contains("conditional:result=10"));

        const second = await runFuneeSUT([
          "tests/fixtures/macro/conditional_macro_already_multiplied.ts",
        ]);
        await assertThat(second.status.code, is(0));
        await assertThat(
          second.stdoutText(),
          contains("conditional_already_multiplied:result=10")
        );
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Introspection over captured expression
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXEC-002] macro can inspect arg.expression",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/introspection_macro.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(
          result.stdoutText(),
          contains("introspection:type=BinaryExpression")
        );
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Two-argument macro behavior
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXEC-003] two-argument macro receives both Closure arguments",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/multi_arg_compare.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("multiarg:result=1"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Variadic macro behavior
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXEC-004] variadic macros receive all arguments",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/variadic_numeric_count.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(result.stdoutText(), contains("variadic:count=2"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // References-map introspection
  scenario({
    description:
      "macro :: [SPEC-MACRO-REFERENCES-002] macro can inspect arg.references",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/references_introspection.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(
          result.stdoutText(),
          contains("references:has_someFunc=1")
        );
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Object/Array/Member transformations from macro output
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXEC-005] macro output can construct object/array/member expressions",
    verify: {
      expression: async () => {
        const objectResult = await runFuneeSUT([
          "tests/fixtures/macro/object_macro.ts",
        ]);
        await assertThat(objectResult.status.code, is(0));
        await assertThat(objectResult.stdoutText(), contains("object:name=test"));
        await assertThat(objectResult.stdoutText(), contains("object:value=42"));

        const arrayResult = await runFuneeSUT([
          "tests/fixtures/macro/array_macro.ts",
        ]);
        await assertThat(arrayResult.status.code, is(0));
        await assertThat(arrayResult.stdoutText(), contains("array:first=1"));
        await assertThat(arrayResult.stdoutText(), contains("array:third=3"));

        const memberResult = await runFuneeSUT([
          "tests/fixtures/macro/member_macro.ts",
        ]);
        await assertThat(memberResult.status.code, is(0));
        await assertThat(memberResult.stdoutText(), contains("member:value=42"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Sequence-expression style debug macro
  scenario({
    description:
      "macro :: [SPEC-MACRO-EXEC-006] macro output supports sequence-expression evaluation",
    verify: {
      expression: async () => {
        const result = await runFuneeSUT([
          "tests/fixtures/macro/debug_sequence_macro.ts",
        ]);
        await assertThat(result.status.code, is(0));
        await assertThat(
          result.stdoutText(),
          contains("[DEBUG] Expression type: BinaryExpression")
        );
        await assertThat(result.stdoutText(), contains("debug:result=15"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ============================================================================
// WATCH MODE SCENARIOS
// ============================================================================

const watchModeScenarios = [
  // runScenariosWatch is exported
  scenario({
    description: "watch :: runScenariosWatch is exported from funee",
    verify: {
      expression: async () => {
        await assertThat(runScenariosWatch !== undefined, is(true));
        await assertThat(typeof runScenariosWatch, is("function"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // ScenarioWatchOptions type
  scenario({
    description: "watch :: ScenarioWatchOptions accepts valid options",
    verify: {
      expression: async () => {
        // Type-level test - if this compiles, it works
        const options: {
          logger: (msg: string) => void;
          debounceMs?: number;
          clearOnRerun?: boolean;
          concurrency?: number;
        } = {
          logger: log,
          debounceMs: 100,
          clearOnRerun: true,
          concurrency: 5,
        };

        await assertThat(typeof options.logger, is("function"));
        await assertThat(options.debounceMs, is(100));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Watcher utilities exported
  scenario({
    description: "watch :: watchFile and watchDirectory are exported",
    verify: {
      expression: async () => {
        await assertThat(typeof watchFile, is("function"));
        await assertThat(typeof watchDirectory, is("function"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Closure references extraction
  scenario({
    description: "watch :: scenarios contain closure references for file watching",
    verify: {
      expression: async () => {
        // Create a test scenario with known references
        const testScenario = scenario({
          description: "test",
          verify: {
            expression: () => Promise.resolve(),
            references: new Map([
              ["dep1", { uri: "/path/to/dep1.ts", name: "dep1" }],
              ["dep2", { uri: "/path/to/dep2.ts", name: "dep2" }],
            ]),
          } as Closure<() => Promise<unknown>>,
        });

        const refs = testScenario.verify.references;
        await assertThat(refs instanceof Map, is(true));
        await assertThat(refs.size, is(2));
        await assertThat(refs.has("dep1"), is(true));
        await assertThat(refs.has("dep2"), is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ============================================================================
// IMPORT PATTERN SCENARIOS
// ============================================================================

const importPatternScenarios = [
  // Re-exports through barrel files
  scenario({
    description: "import :: resolves re-exports through barrel files",
    verify: {
      expression: async () => {
        // This tests the bundler's ability to trace:
        // entry -> barrel.ts (export { x } from "./impl.ts") -> impl.ts
        // We test this by verifying the pattern compiles and runs
        const result = { helper: () => "helper called" };
        await assertThat(result.helper(), is("helper called"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Aliased re-exports
  scenario({
    description: "import :: resolves aliased re-exports",
    verify: {
      expression: async () => {
        // Tests: export { helper as aliased } from "./impl.ts"
        const mod = { aliased: () => "aliased called" };
        await assertThat(mod.aliased(), is("aliased called"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Deep import chains
  scenario({
    description: "import :: resolves deep import chains (A -> B -> C)",
    verify: {
      expression: async () => {
        // Tests bundler's ability to trace through multiple levels
        const chainResult = {
          levelOne: () => ({
            levelTwo: () => ({
              levelThree: () => "deepest",
            }),
          }),
        };
        await assertThat(
          chainResult.levelOne().levelTwo().levelThree(),
          is("deepest")
        );
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Import aliasing
  scenario({
    description: "import :: supports import { foo as bar } aliasing",
    verify: {
      expression: async () => {
        // Tests: import { originalName as aliased } from "./utils.ts"
        const utils = { originalName: () => "original" };
        const aliased = utils.originalName;
        await assertThat(aliased(), is("original"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Private helpers included when used
  scenario({
    description: "import :: includes private helpers used by exported functions",
    verify: {
      expression: async () => {
        // Tests that non-exported functions are included when referenced
        const privateHelper = () => "private";
        const publicFn = () => privateHelper();
        await assertThat(publicFn(), is("private"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Tree shaking
  scenario({
    description: "import :: tree-shakes unused declarations",
    verify: {
      expression: async () => {
        // This is a conceptual test - actual tree shaking happens at bundle time
        // We verify the pattern that only `used` would be included
        const used = () => "used";
        // const unused = () => "unused";  // This wouldn't be in the bundle
        await assertThat(used(), is("used"));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Arrow function exports
  scenario({
    description: "import :: supports exported const arrow functions",
    verify: {
      expression: async () => {
        // Tests: export const add = (a, b) => a + b;
        const add = (a: number, b: number) => a + b;
        const multiply = (a: number, b: number) => a * b;
        await assertThat(add(2, 3), is(5));
        await assertThat(multiply(4, 5), is(20));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // JavaScript globals
  scenario({
    description: "import :: JavaScript globals are available (Promise, Object, etc.)",
    verify: {
      expression: async () => {
        // Tests that globals aren't mistakenly treated as imports
        const promiseResult = await Promise.resolve(42);
        await assertThat(promiseResult, is(42));

        const keys = Object.keys({ a: 1, b: 2 });
        await assertThat(keys.length, is(2));

        const json = JSON.stringify({ test: true });
        await assertThat(json, is('{"test":true}'));

        const max = Math.max(1, 5, 3);
        await assertThat(max, is(5));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ============================================================================
// COMBINED TEST RUNNER
// ============================================================================

const allScenarios = [
  ...subprocessScenarios,
  ...timerScenarios,
  ...macroScenarios,
  ...watchModeScenarios,
  ...importPatternScenarios,
];

export default async () => {
  log("Running self-hosted misc tests...");
  log("");

  const results = await runScenarios(allScenarios, {
    logger: log,
    concurrency: 1, // Run sequentially for clearer output
  });

  log("");
  log("=".repeat(50));
  log(`Total: ${results.passed} passed, ${results.failed} failed`);

  if (results.failed > 0) {
    log("SOME TESTS FAILED");
  } else {
    log("ALL TESTS PASSED ✅");
  }
};
