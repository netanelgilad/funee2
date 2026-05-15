import { assertion, Assertion } from "./Assertion.ts";
import { AssertionError } from "./AssertionError.ts";

export type EventuallyOptions = {
  timeoutMs?: number;
  intervalMs?: number;
};

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

const buildEventually = <T>(
  expected: Assertion<T>,
  options: EventuallyOptions = {},
): Assertion<() => T | Promise<T>> => {
  return assertion(async (actual: () => T | Promise<T>) => {
    const timeoutMs = options.timeoutMs ?? 5000;
    const intervalMs = options.intervalMs ?? 100;
    const startedAt = Date.now();
    let lastError: unknown;

    while (Date.now() - startedAt < timeoutMs) {
      try {
        await expected(await actual());
        return;
      } catch (err) {
        lastError = err;
      }

      await sleep(intervalMs);
    }

    const lastMessage = lastError instanceof Error
      ? lastError.message
      : String(lastError);

    throw AssertionError({
      message: "Expected function to eventually satisfy assertion within " +
        timeoutMs + "ms. Last failure: " + lastMessage,
      actual,
      expected,
      operator: "eventually",
    });
  });
};

type Eventually = {
  <T>(expected: Assertion<T>): Assertion<() => T | Promise<T>>;
  (options: EventuallyOptions): <T>(expected: Assertion<T>) => Assertion<() => T | Promise<T>>;
};

export const eventually: Eventually = (expectedOrOptions: any): any => {
  if (typeof expectedOrOptions === "function") {
    return buildEventually(expectedOrOptions);
  }

  return (expected: Assertion<any>) => buildEventually(expected, expectedOrOptions);
};
