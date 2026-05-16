import { mkdir, removeFile, tmpdir, writeFile } from "host://fs";
import type { Closure } from "../core.ts";

export type DefaultExportFixtureOptions = {
  fixtureDir?: string;
  moduleSpecifier?: (uri: string) => string;
};

export type DefaultExportFixture = {
  path: string;
  [Symbol.asyncDispose](): Promise<void>;
};

const unwrapFsResult = (operation: string, result: string | void) => {
  if (typeof result !== "string") {
    return;
  }

  const parsed = JSON.parse(result) as { type: "ok" } | {
    type: "error";
    error: string;
  };

  if (parsed.type === "error") {
    throw new Error(`${operation} failed: ${parsed.error}`);
  }
};

export const defaultExportFixture = async <T>(
  value: Closure<T>,
  options: DefaultExportFixtureOptions = {},
): Promise<DefaultExportFixture> => {
  const fixtureDir = options.fixtureDir ?? `${tmpdir()}/funee-fixtures`;
  const fixturePath = `${fixtureDir}/${Date.now()}-${Math.random().toString(36).slice(2)}.ts`;
  const moduleSpecifier = options.moduleSpecifier ?? ((uri: string) => uri);
  const imports = Array.from(value.references.entries()).map(
    ([localName, reference]) =>
      `import { ${reference.name} as ${localName} } from ${JSON.stringify(moduleSpecifier(reference.uri))};`,
  );
  const code = value.code ?? String(value.expression).trim();
  const moduleSource = [
    ...imports,
    "",
    `export default ${code};`,
    "",
  ].join("\n");

  unwrapFsResult("mkdir", mkdir(fixtureDir, true));
  unwrapFsResult("writeFile", writeFile(fixturePath, moduleSource));

  return {
    path: fixturePath,
    async [Symbol.asyncDispose]() {
      unwrapFsResult("removeFile", removeFile(fixturePath));
    },
  };
};
