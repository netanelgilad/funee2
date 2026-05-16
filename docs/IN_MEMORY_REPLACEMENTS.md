# In-Memory Replacements

## Goal

Self-hosted tests often run the system under test as an external `funee` process,
for example `tests/self-hosted/gateway.ts` spawning `./target/sut/funee` with a
fixture entrypoint. In that shape, replacements cannot be passed as in-process
JavaScript objects to `runScenarios` because the code under test runs in a
separate runtime.

## Proposed Shape

Add a CLI option that points `funee` at a replacements module before executing the
entrypoint:

```sh
funee --replacements tests/fixtures/gateway/replacements.ts tests/fixtures/gateway/ai-gateway-v0.ts
```

The replacements module exports replacement tuples. Each tuple identifies a node
in the source graph and provides another implementation node. Before execution,
`funee` should rewrite the source graph so references to the original node point
at the replacement node.

Example shape:

```ts
import { log } from "host://console";
import { replacement } from "../../funee-lib/replacements/index.ts";

export default [
  replacement(log, (message: string) => {
    // in-memory behavior for this test runtime
  }),
];
```

## Source Graph Semantics

Replacements are not runtime monkey patches. They should not work by installing a
global replacement map or by having host preambles check runtime state.

The intended behavior is:

1. Build the source graph for the target entrypoint.
2. Build/load the replacement declarations from the replacements module.
3. For each `replacement(target, implementation)` tuple, resolve `target` to its
   canonical graph node.
4. Insert or reuse a graph node for `implementation`.
5. Redirect graph edges that point at the target node so they point at the
   replacement node before emitting/executing JavaScript.

This keeps funee's execution model statically analyzable: the executed program is
the rewritten source graph, not the original graph plus runtime interception.

## First Behavior To Drive

A self-hosted test should spawn the SUT `funee` process with `--replacements`
pointing at a fixture replacements file. The fixture entrypoint calls a host
function such as `host://console` `log`. The replacement implementation should run
instead of the real host implementation, and the test should observe that through
an in-memory/test-controlled side effect.

## Notes

- The replacement must be loaded by the CLI before the target graph is emitted or
  executed.
- The replacement file should affect source graph construction/rewriting, not
  runtime behavior after emission.
- The API should stay close to the original `in_memory_host` idea: replacement
  tuples pair a canonical target with a replacement implementation.

## In-Memory Host Fixture Direction

The reusable in-memory host should live in library code. Test-specific files
should only configure that host for a scenario or group of scenarios, for example
declaring HTTP fetch routes while reusing the shared fetch implementation.

Desired shape:

```ts
import { createInMemoryHost } from "../../../funee-lib/in-memory-host/index.ts";

export default createInMemoryHost({
  http: {
    servers: [
      {
        origin: "https://upstream.example.test",
        handler: async (request: Request) => {
          if (new URL(request.url).pathname === "/message") {
            return Response.json({ message: "from reusable in-memory host" });
          }

          return new Response("Not found", { status: 404 });
        },
      },
    ],
  },
});
```

`createInMemoryHost` should stay focused on host-level concerns. For HTTP, that
means mapping an origin/domain to a request handler, similar to mounting an
in-memory server. It should not own application-level routing such as matching
`/message`; that belongs inside the configured handler. The same boundary should
apply elsewhere: the in-memory host provides low-level host capabilities and
state, while test/application-specific semantics stay in fixture configuration.

Later ergonomic layer: keep this configuration in the self-hosted test itself and
use a macro/helper to generate a temporary replacements module for the spawned
SUT. That would preserve go-to-definition and avoid stringly relative fixture
paths while still feeding the CLI a real replacement module across the process
boundary.
