import type { CanonicalName, Closure } from "../core.ts";
import { createMacro } from "../core.ts";

export type Replacement<T extends (...args: Array<any>) => any> = {
  target: CanonicalName;
  implementation: T;
};

export const replacement = createMacro(
  // @ts-expect-error funee macros can receive multiple captured arguments.
  <T extends (...args: Array<any>) => any>(
    target: Closure<T>,
    implementation: Closure<T>,
  ): Closure<Replacement<T>> => {
    const targetName = String(target.expression).trim();
    const canonicalTarget = target.references.get(targetName);

    if (!canonicalTarget) {
      throw new Error(
        `replacement: target "${targetName}" was not found in closure references`,
      );
    }

    return {
      expression:
        `({ target: { uri: ${JSON.stringify(canonicalTarget.uri)}, name: ${JSON.stringify(canonicalTarget.name)} }, ` +
        `implementation: ${String(implementation.expression).trim()} })`,
      references: implementation.references,
    };
  },
) as <
  T extends (...args: Array<any>) => any,
>(target: T, implementation: T) => Replacement<T>;
