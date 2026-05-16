/**
 * closure macro - Capture an expression as Closure<Closure<T>>
 * 
 * This is the core macro that enables runtime access to AST.
 * It captures an expression and returns code that constructs
 * a Closure object at runtime containing the expression's AST.
 */

import type { Closure, CanonicalName } from "../core.ts";
import { createMacro } from "../core.ts";

/**
 * The closure macro captures an expression and returns its AST at runtime.
 * 
 * At bundle time:
 *   closure(x => x + 1)
 * 
 * Becomes:
 *   Closure({
 *     expression: { type: "ArrowFunctionExpression", code: "(x) => x + 1" },
 *     references: new Map([...])
 *   })
 */
export const closure = createMacro(<T>(nodeClosure: Closure<T>): Closure<Closure<T>> => {
  // nodeClosure.expression is a CODE STRING like "(a, b) => a + b"
  // We need to determine the AST type from the code
  const code = String(nodeClosure.expression).trim();
  
  // Build the references array entries as code
  const refsEntries = Array.from(nodeClosure.references.entries()).map(
    ([localName, canonicalName]) => 
      `[${JSON.stringify(localName)}, { uri: ${JSON.stringify(canonicalName.uri)}, name: ${JSON.stringify(canonicalName.name)} }]`
  );
  
  const refsCode = refsEntries.length > 0 
    ? `new Map([${refsEntries.join(", ")}])`
    : "new Map()";

  let escapedCode = "";
  let quote: string | undefined;
  let escaped = false;
  for (const ch of code) {
    if (escaped) {
      escapedCode += ch;
      escaped = false;
    } else if (ch === "\\") {
      escapedCode += ch;
      escaped = true;
    } else if (quote) {
      if (ch === quote) {
        quote = undefined;
        escapedCode += ch;
      } else if (ch === "\n") {
        escapedCode += "\\n";
      } else if (ch === "\r") {
        escapedCode += "\\r";
      } else {
        escapedCode += ch;
      }
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      escapedCode += ch;
    } else {
      escapedCode += ch;
    }
  }

  const expressionCode = code.includes("=>") || code.startsWith("function") ||
      code.startsWith("async function")
    ? `Object.assign(${escapedCode}, { type: "ArrowFunctionExpression" })`
    : escapedCode;

  // Build a runtime Closure object. Validator scenarios execute expression directly,
  // while fixtures use `code` to write the original source into generated modules.
  const resultCode = `({ expression: ${expressionCode}, code: ${JSON.stringify(code)}, references: ${refsCode} })`;

  return {
    expression: resultCode,
    references: new Map<string, CanonicalName>(),
  };
});
