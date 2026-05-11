/**
 * runScenarios - Execute test scenarios and report results.
 *
 * Runs an array of scenarios, respecting focus flags, and logs
 * results to the console. Supports concurrent execution.
 *
 * @example
 * import { runScenarios, scenario, log, Closure, assertThat, is } from "funee";
 *
 * const scenarios = [
 *   scenario({
 *     description: "math works",
 *     verify: {
 *       expression: async () => { await assertThat(1 + 1, is(2)); },
 *       references: new Map(),
 *     } as Closure<() => Promise<unknown>>,
 *   }),
 * ];
 *
 * await runScenarios(scenarios, { logger: log });
 */

import type { Scenario } from "./scenario.ts";
import { log as hostLog } from "host://console";

/**
 * Result of running a single scenario.
 */
export type ScenarioResult = {
  description: string;
  success: boolean;
  error?: unknown;
};

/**
 * Logger function type for scenario output.
 */
export type ScenarioLogger = (message: string) => void;

/**
 * Options for runScenarios.
 */
export type RunScenariosOptions = {
  /** Maximum concurrent scenarios (default: 10) */
  concurrency?: number;
  /** Logger function (default: host://console log) */
  logger?: ScenarioLogger;
};

/**
 * Executes an array of scenarios and reports results.
 *
 * - If any scenarios have `focus: true`, only those run
 * - Scenarios run concurrently (configurable)
 * - Results are logged with ✅ or ❌ indicators
 *
 * @param scenarios - Array of scenarios to run
 * @param options - Configuration with required logger
 * @returns Promise that resolves when all scenarios complete
 */
export const runScenarios = async (
  scenarios: Array<Scenario>,
  options: RunScenariosOptions = {},
): Promise<Array<ScenarioResult>> => {
  const { concurrency = 10, logger = hostLog } = options;

  // Filter to focused scenarios if any exist
  const focusedScenarios = scenarios.filter((x) => x.focus);
  const scenariosToRun =
    focusedScenarios.length > 0 ? focusedScenarios : scenarios;

  if (focusedScenarios.length > 0) {
    logger(`⚠️  Running ${focusedScenarios.length} focused scenario(s) only`);
  }

  const results: Array<ScenarioResult> = [];

  // Run scenarios sequentially (simple and reliable)
  for (const scenario of scenariosToRun) {
    logger(`🏃 ${scenario.description}`);

    try {
      // Execute the verify function from the closure
      const verifyFn = scenario.verify.expression;
      await verifyFn();

      logger(`✅  ${scenario.description}`);
      results.push({ description: scenario.description, success: true });
    } catch (err) {
      logger(`❌  ${scenario.description}`);
      logger("");
      logger(String(err));
      if (err instanceof Error && err.stack) {
        logger(err.stack);
      }
      logger("");
      results.push({
        description: scenario.description,
        success: false,
        error: err,
      });
    }
  }

  // Summary
  const passed = results.filter((r) => r.success).length;
  const failed = results.filter((r) => !r.success).length;

  logger("");
  logger(
    `📊 Results: ${passed} passed, ${failed} failed, ${results.length} total`,
  );

  if (failed > 0) {
    logger("");
    logger("Failed scenarios:");
    for (const result of results.filter((r) => !r.success)) {
      logger(`  ❌ ${result.description}`);
    }
  }

  return results;
};
