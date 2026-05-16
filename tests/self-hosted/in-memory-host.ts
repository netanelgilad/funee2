/// <reference path="../../funee-lib/host/process.d.ts" />

import { exists } from "host://fs";
import { log } from "host://console";
import { fetch } from "host://http";
import { scenario, runScenarios } from "../../funee-lib/validator/index.ts";
import { closure } from "../../funee-lib/macros/closure.ts";
import { assertThat, is, otherwise } from "../../funee-lib/assertions/index.ts";
import { buffer } from "../../funee-lib/streams/buffer.ts";
import {
  createInMemoryHost,
  inMemoryHostFixture,
} from "../../funee-lib/in-memory-host/index.ts";
import { spawnFuneeSUT } from "./_sut.ts";

const inlineUpstreamHost = closure({
  http: {
    servers: [
      {
        origin: "https://upstream.example.test",
        handler: async (request: Request) => {
          if (new URL(request.url).pathname === "/message") {
            return Response.json({ message: "from inline in-memory host" });
          }

          return new Response("Not found", { status: 404 });
        },
      },
    ],
  },
});

const inlineFetchEntry = closure(async () => {
  const response = await fetch("https://upstream.example.test/message");
  const payload = await response.json() as { message: string };

  log(`upstream:${payload.message}`);
});

const inlineUpstreamReplacements = closure(createInMemoryHost({
  http: {
    servers: [
      {
        origin: "https://upstream.example.test",
        handler: async (request: Request) => {
          if (new URL(request.url).pathname === "/message") {
            return Response.json({ message: "from default export fixture" });
          }

          return new Response("Not found", { status: 404 });
        },
      },
    ],
  },
}));

const scenarios = [
  scenario({
    description: "in-memory host :: configured fetch route replaces host fetch",
    verify: closure(async () => {
      await using result = await spawnFuneeSUT({
        entrypoint: "tests/fixtures/in-memory-host/fetch-entry.ts",
        inMemoryHost: "tests/fixtures/in-memory-host/http-fetch-fixture.ts",
      });
      const stdout = buffer(result.stdout);
      const stderr = buffer(result.stderr);

      const { status } = await result.completion();

      await assertThat(
        status,
        is(0),
        otherwise(
          () =>
            "Spawned process output:\n\nstdout:\n" +
            stdout.text() +
            "\n\nstderr:\n" +
            stderr.text(),
        ),
      );
      await assertThat(stdout.text(), is("upstream:from reusable in-memory host\n"));
    }),
  }),
  scenario({
    description:
      "in-memory host :: inline config writes replacements module for spawned runtime",
    verify: closure(async () => {
      await using fixture = await inMemoryHostFixture(inlineUpstreamHost);

      await using result = await spawnFuneeSUT({
        entrypoint: "tests/fixtures/in-memory-host/fetch-entry.ts",
        inMemoryHost: fixture.path,
      });
      const stdout = buffer(result.stdout);

      const { status } = await result.completion();

      await assertThat(status, is(0));
      await assertThat(stdout.text(), is("upstream:from inline in-memory host\n"));
    }),
  }),
  scenario({
    description: "in-memory host :: fixture deletes generated module after using",
    verify: closure(async () => {
      let replacementsPath = "";

      {
        await using fixture = await inMemoryHostFixture(inlineUpstreamHost);
        replacementsPath = fixture.path;

        await assertThat(exists(replacementsPath), is(true));
      }

      await assertThat(exists(replacementsPath), is(false));
    }),
  }),
  scenario({
    description:
      "fixtures :: closure default exports can replace spawned entrypoint and replacements paths",
    verify: closure(async () => {
      await using result = await spawnFuneeSUT({
        entrypoint: inlineFetchEntry,
        inMemoryHost: inlineUpstreamReplacements,
      });
      const stdout = buffer(result.stdout);
      const stderr = buffer(result.stderr);

      const { status } = await result.completion();

      await assertThat(
        status,
        is(0),
        otherwise(
          () =>
            "Spawned process output:\n\nstdout:\n" +
            stdout.text() +
            "\n\nstderr:\n" +
            stderr.text(),
        ),
      );
      await assertThat(stdout.text(), is("upstream:from default export fixture\n"));
    }),
  }),
];

export default async () => {
  await runScenarios(scenarios);
};
