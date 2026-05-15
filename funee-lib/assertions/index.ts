/**
 * funee assertions - A testing library for funee applications.
 * 
 * Provides composable assertion primitives for testing synchronous and
 * asynchronous code.
 * 
 * @example
 * import { assertThat, is, notAssertion, both, otherwise } from "funee";
 * 
 * // Basic equality
 * await assertThat(2 + 2, is(4));
 * 
 * // Negation
 * await assertThat(5, notAssertion(is(10)));
 * 
 * // Combining assertions
 * await assertThat(value, both(isNumber, isPositive));
 * 
 * // With error context
 * await assertThat(result, is(expected), otherwise((err) => `Context: ${context}`));
 */

// Core types and helpers
export type { Assertion, Otherwise, Within, OtherwiseCallback } from "./Assertion.ts";
export { assertion, otherwise, within, isOtherwise, isWithin } from "./Assertion.ts";

// AssertionError for catching assertion failures
export type { AssertionErrorOptions, AssertionErrorType } from "./AssertionError.ts";
export { AssertionError, isAssertionError, assert, strictEqual, deepEqual } from "./AssertionError.ts";

// Main assertion function
export { assertThat } from "./assertThat.ts";

// Assertion combinators
export { is } from "./is.ts";
export { not } from "./not.ts";
export { both } from "./both.ts";
export type { EventuallyOptions } from "./eventually.ts";
export { eventually } from "./eventually.ts";

// Matchers
export { contains } from "./contains.ts";
export { matches } from "./matches.ts";
export { greaterThan } from "./greaterThan.ts";
export { lessThan } from "./lessThan.ts";
export { greaterThanOrEqual } from "./greaterThanOrEqual.ts";
export { lessThanOrEqual } from "./lessThanOrEqual.ts";
