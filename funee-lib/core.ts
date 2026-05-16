/**
 * funee - Core Runtime Library
 * 
 * This module provides the foundational types and functions for the funee macro system.
 * Users import these to define and work with compile-time macros.
 */

/**
 * Closure<T> - Represents a captured expression with its external references
 * 
 * A Closure captures the AST of an expression along with all references
 * to definitions outside its scope. This is the fundamental building block
 * for compile-time macros in funee.
 * 
 * @template T - The TypeScript type of the captured expression
 * 
 * @example
 * ```typescript
 * const add = (a: number, b: number) => a + b;
 * const addClosure: Closure<typeof add> = closure(add);
 * // addClosure.expression contains the arrow function AST
 * // addClosure.references contains external refs (empty for this example)
 * ```
 */
export interface Closure<T> {
  /**
   * The AST node representing the captured expression.
   * At runtime, this is a serialized AST structure.
   * At bundle time, this is the actual SWC/Babel AST node.
   */
  expression: any;  // AST node (type depends on AST library used)
  code?: string;
  
  /**
   * Map of external references used in the expression.
   * Key: local variable name in the expression
   * Value: CanonicalName pointing to the definition
   * 
   * @example
   * ```typescript
   * // For: (x) => x * multiplier
   * // references = Map({ "multiplier": { uri: "./math.ts", name: "multiplier" } })
   * ```
   */
  references: Map<string, CanonicalName>;
}

/**
 * CanonicalName - Unique identifier for a definition across the entire codebase
 * 
 * A CanonicalName uniquely identifies a definition (variable, function, etc.)
 * by combining its module URI with its export name. This allows the bundler
 * to track references across module boundaries.
 * 
 * @example
 * ```typescript
 * const name: CanonicalName = {
 *   uri: "https://example.com/math.ts",
 *   name: "add"
 * };
 * ```
 */
export interface CanonicalName {
  /**
   * Module URI where the definition lives.
   * Can be a file path, HTTP URL, or package specifier.
   * 
   * @example
   * - "./utils.ts"
   * - "https://deno.land/std/path/mod.ts"
   * - "@opah/core"
   * - "funee"
   */
  uri: string;
  
  /**
   * The export name of the definition within the module.
   * For default exports, this is typically "default".
   * 
   * @example
   * - "add" (for: export const add = ...)
   * - "default" (for: export default ...)
   */
  name: string;
}

/**
 * createMacro<T, R> - Marks a function as a compile-time macro
 * 
 * Functions wrapped with createMacro are executed at BUNDLE TIME, not runtime.
 * The bundler detects these markers and executes the macro function during
 * the bundling process, replacing call sites with the transformed AST.
 * 
 * @template T - Input type (TypeScript type of the argument)
 * @template R - Return type (TypeScript type of the result)
 * 
 * @param fn - The macro transformation function that runs at bundle time.
 *             Receives Closure objects as arguments and returns a Closure.
 * 
 * @returns A function with the same signature as T -> R, but behavior is
 *          intercepted by the bundler. If somehow called at runtime, throws.
 * 
 * @example
 * ```typescript
 * // Define a macro that captures expressions as Closures
 * const closure = createMacro(<T>(input: Closure<T>): Closure<Closure<T>> => {
 *   // This runs at BUNDLE TIME
 *   // Return a Closure that constructs the input Closure at runtime
 *   return {
 *     expression: constructClosureAST(input),
 *     references: new Map([["Closure", { uri: "funee", name: "Closure" }]])
 *   };
 * });
 * 
 * // Use the macro (this is transformed at bundle time)
 * const add = (a, b) => a + b;
 * const addClosure = closure(add);  // Bundler intercepts this call
 * ```
 */
export const createMacro = <T, R>(
  fn: (closure: Closure<T>) => Closure<R>
): ((value: T) => R) => {
  // This function should NEVER be called at runtime
  // The bundler intercepts all calls to macro-marked functions
  // If this executes, it means the bundler failed to expand the macro
  throw new Error(
    "createMacro was not expanded by the bundler. " +
    "This indicates a bug in the macro system or incorrect usage."
  );
};

/**
 * Closure constructor function - Creates Closure objects at runtime
 * 
 * This is used by the bundler-generated code to construct Closure objects
 * from serialized AST and references. The bundler emits calls to this
 * function when macro transformations produce Closure values.
 * 
 * @template T - TypeScript type of the captured expression
 * 
 * @param data - Object containing expression AST and references
 * @param data.expression - The AST node (usually a serialized object)
 * @param data.references - References map (can be a plain object or Map)
 * 
 * @returns A properly formatted Closure<T> object
 * 
 * @example
 * ```typescript
 * // Bundler-generated code might look like:
 * const addClosure = Closure({
 *   expression: { type: "ArrowFunctionExpression", params: [...], body: {...} },
 *   references: { multiplier: { uri: "./math.ts", name: "multiplier" } }
 * });
 * ```
 */
export const Closure = <T>(data: {
  expression: any;
  references: any;
}): Closure<T> => ({
  expression: data.expression,
  // Handle both Map and plain object formats for references
  references: data.references instanceof Map
    ? data.references
    : new Map(Object.entries(data.references || {}))
});

/**
 * MacroFunction type - The signature of a compile-time macro transformation
 * 
 * This type describes functions that execute at bundle time to transform code.
 * 
 * @template T - Input type (what the macro accepts)
 * @template R - Return type (what the macro produces)
 */
export type MacroFunction<T = any, R = any> = (
  closure: Closure<T>
) => Closure<R> | [Closure<R>, Map<CanonicalName, any>];

/**
 * Helper type for macro results that include artificial definitions
 * 
 * Some macros need to inject new definitions into the module scope.
 * They can return a tuple of [Closure, Definitions] instead of just a Closure.
 */
export type MacroResultWithDefinitions<R> = [
  Closure<R>,
  Map<CanonicalName, any>
];
