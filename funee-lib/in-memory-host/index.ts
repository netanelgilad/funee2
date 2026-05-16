import { fetch } from "host://http";
import { mkdir, removeFile, tmpdir, writeFile } from "host://fs";
import { env } from "host://process";
import type { Closure } from "../core.ts";
import { replacement } from "../replacements/index.ts";

export type InMemoryHttpServer = {
  origin: string;
  handler: (request: Request) => Response | Promise<Response>;
};

export type InMemoryHostConfig = {
  http?: {
    servers?: Array<InMemoryHttpServer>;
  };
};

export type InMemoryHostFixtureOptions = {
  createInMemoryHostModule?: string;
  fixtureDir?: string;
  moduleSpecifier?: (uri: string) => string;
};

export type InMemoryHostFixture = {
  path: string;
  [Symbol.asyncDispose](): Promise<void>;
};

export const createInMemoryHost = (config: InMemoryHostConfig) => {
  const servers = config.http?.servers ?? [];

  return [
    replacement(fetch, async (input: string | URL | Request, init?: RequestInit) => {
      const request = input instanceof Request
        ? input
        : new Request(input, init);
      const requestOrigin = new URL(request.url).origin;
      const server = servers.find((server) => server.origin === requestOrigin);

      if (!server) {
        return new Response(`No in-memory HTTP server for ${requestOrigin}`, {
          status: 502,
        });
      }

      return await server.handler(request);
    }),
  ];
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

const defaultCreateInMemoryHostModule = () =>
  `${env("PWD") ?? "."}/funee-lib/in-memory-host/index.ts`;

export const inMemoryHostFixture = async (
  config: Closure<InMemoryHostConfig>,
  options: InMemoryHostFixtureOptions = {},
): Promise<InMemoryHostFixture> => {
  const fixtureDir = options.fixtureDir ??
    `${tmpdir()}/funee-in-memory-host-fixtures`;
  const fixturePath = `${fixtureDir}/${Date.now()}-${Math.random().toString(36).slice(2)}.ts`;
  const moduleSpecifier = options.moduleSpecifier ?? ((uri: string) => uri);
  const createInMemoryHostModule = options.createInMemoryHostModule ??
    defaultCreateInMemoryHostModule();
  const imports = Array.from(config.references.entries()).map(
    ([localName, reference]) =>
      `import { ${reference.name} as ${localName} } from ${JSON.stringify(moduleSpecifier(reference.uri))};`,
  );
  const configCode = config.code ?? String(config.expression).trim();
  const moduleSource = [
    `import { createInMemoryHost } from ${JSON.stringify(createInMemoryHostModule)};`,
    ...imports,
    "",
    `export default createInMemoryHost(${configCode});`,
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
