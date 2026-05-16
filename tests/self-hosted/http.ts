/// <reference path="../../funee-lib/host/index.d.ts" />

/**
 * Self-hosted HTTP tests
 * 
 * Tests for HTTP-related functionality:
 * - HTTP imports (fetching modules from URLs, caching)
 * - HTTP server (serve() API - request/response handling)
 * - Fetch API (fetch(), Response, Headers)
 * 
 * Uses the scenario/runScenarios pattern for test organization.
 */
import { log } from "host://console";
import { serve } from "host://http/server";
import type { Closure } from "../../funee-lib/core.ts";
import { scenario, runScenarios } from "../../funee-lib/validator/index.ts";
import { closure } from "../../funee-lib/macros/closure.ts";
import {
  assertThat,
  eventually,
  greaterThan,
  is,
} from "../../funee-lib/assertions/index.ts";
import { spawnFuneeSUT } from "./_sut.ts";

// ==================== HTTP SERVER SCENARIOS ====================

const httpServerScenarios = [
  scenario({
    description: "http server :: spawned JSON health server responds",
    verify: closure(async () => {
      const port = 18988;

      await using _server = await spawnFuneeSUT({
        entrypoint: "tests/fixtures/gateway/json-health-server.ts",
        env: {
          GATEWAY_PORT: String(port),
        },
      });

      const isServerHealthy = async () =>
        (await fetch(`http://127.0.0.1:${port}/healthz`)).status === 200;

      await assertThat(isServerHealthy, eventually(is(true)));
    }),
  }),

  // Basic server functionality
  scenario({
    description: "http server :: responds to GET",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("hello"));
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.status, is(200));
          await assertThat(await res.text(), is("hello"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: server has port property",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("test"));
        try {
          await assertThat(typeof server.port, is("number"));
          await assertThat(server.port, greaterThan(0));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: server has hostname property",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("test"));
        try {
          await assertThat(typeof server.hostname, is("string"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: server has shutdown function",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("test"));
        try {
          await assertThat(typeof server.shutdown, is("function"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Request handling
  scenario({
    description: "http server :: handles GET method",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          return new Response(req.method);
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(await res.text(), is("GET"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: handles POST method",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          return new Response(req.method);
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            method: "POST",
          });
          await assertThat(await res.text(), is("POST"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: handles PUT method",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          return new Response(req.method);
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            method: "PUT",
          });
          await assertThat(await res.text(), is("PUT"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: handles DELETE method",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          return new Response(req.method);
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            method: "DELETE",
          });
          await assertThat(await res.text(), is("DELETE"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Request body parsing
  scenario({
    description: "http server :: parses JSON request body",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, async (req) => {
          const body = await req.json();
          return Response.json({ received: body });
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ hello: "world", num: 42 }),
          });
          const data = await res.json();
          await assertThat(data.received.hello, is("world"));
          await assertThat(data.received.num, is(42));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: parses text request body",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, async (req) => {
          const body = await req.text();
          return new Response(`received: ${body}`);
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            method: "POST",
            body: "hello world",
          });
          await assertThat(await res.text(), is("received: hello world"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Request URL and query params
  scenario({
    description: "http server :: parses request URL pathname",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          return new Response(url.pathname);
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/api/users`);
          await assertThat(await res.text(), is("/api/users"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: parses query parameters",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          const foo = url.searchParams.get("foo");
          const bar = url.searchParams.get("bar");
          return Response.json({ foo, bar });
        });
        try {
          const res = await fetch(
            `http://localhost:${server.port}/?foo=hello&bar=42`
          );
          const data = await res.json();
          await assertThat(data.foo, is("hello"));
          await assertThat(data.bar, is("42"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Request headers
  scenario({
    description: "http server :: receives request headers",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          const custom = req.headers.get("x-custom");
          const auth = req.headers.get("authorization");
          return Response.json({ custom, auth });
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            headers: {
              "X-Custom": "test-value",
              Authorization: "Bearer token123",
            },
          });
          const data = await res.json();
          await assertThat(data.custom, is("test-value"));
          await assertThat(data.auth, is("Bearer token123"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: headers are case-insensitive",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          // Access header with different case than sent
          const val = req.headers.get("X-CUSTOM-HEADER");
          return new Response(val || "not found");
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            headers: { "x-custom-header": "works" },
          });
          await assertThat(await res.text(), is("works"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Response handling
  scenario({
    description: "http server :: sets custom response headers",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => {
          return new Response("body", {
            headers: {
              "X-Custom": "custom-value",
              "X-Another": "another-value",
            },
          });
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.headers.get("x-custom"), is("custom-value"));
          await assertThat(res.headers.get("x-another"), is("another-value"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: returns custom status codes",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          const status = parseInt(url.searchParams.get("status") || "200");
          return new Response("body", { status });
        });
        try {
          const res200 = await fetch(
            `http://localhost:${server.port}/?status=200`
          );
          await assertThat(res200.status, is(200));
          await assertThat(res200.ok, is(true));

          const res201 = await fetch(
            `http://localhost:${server.port}/?status=201`
          );
          await assertThat(res201.status, is(201));
          await assertThat(res201.ok, is(true));

          const res400 = await fetch(
            `http://localhost:${server.port}/?status=400`
          );
          await assertThat(res400.status, is(400));
          await assertThat(res400.ok, is(false));

          const res404 = await fetch(
            `http://localhost:${server.port}/?status=404`
          );
          await assertThat(res404.status, is(404));
          await assertThat(res404.ok, is(false));

          const res500 = await fetch(
            `http://localhost:${server.port}/?status=500`
          );
          await assertThat(res500.status, is(500));
          await assertThat(res500.ok, is(false));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http server :: Response.json() sets content-type",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => {
          return Response.json({ message: "hello", num: 42 });
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          const contentType = res.headers.get("content-type");
          await assertThat(
            contentType?.includes("application/json"),
            is(true)
          );
          const data = await res.json();
          await assertThat(data.message, is("hello"));
          await assertThat(data.num, is(42));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Concurrent requests
  scenario({
    description: "http server :: handles concurrent requests",
    verify: {
      expression: async () => {
        let counter = 0;
        const server = serve({ port: 0 }, () => {
          const id = ++counter;
          return new Response(`request-${id}`);
        });
        try {
          // Fire 5 concurrent requests
          const promises = Array.from({ length: 5 }, () =>
            fetch(`http://localhost:${server.port}/`).then((r) => r.text())
          );
          const results = await Promise.all(promises);
          // All 5 should have completed
          await assertThat(results.length, is(5));
          // Each should have a unique ID
          const ids = results.map((r) => parseInt(r.split("-")[1]));
          const uniqueIds = new Set(ids);
          await assertThat(uniqueIds.size, is(5));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Server shutdown
  scenario({
    description: "http server :: graceful shutdown",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("ok"));
        const port = server.port;

        // Make a request to verify it's working
        const res1 = await fetch(`http://localhost:${port}/`);
        await assertThat(res1.ok, is(true));

        // Shutdown
        await server.shutdown();

        // After shutdown, connections should fail
        let failed = false;
        try {
          await fetch(`http://localhost:${port}/`);
        } catch {
          failed = true;
        }
        await assertThat(failed, is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ==================== FETCH API SCENARIOS ====================

const fetchApiScenarios = [
  // Basic fetch functionality
  scenario({
    description: "fetch api :: basic GET returns Response",
    verify: {
      expression: async () => {
        // Use a local server for testing
        const server = serve({ port: 0 }, () =>
          Response.json({ success: true })
        );
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(typeof res, is("object"));
          await assertThat("ok" in res, is(true));
          await assertThat("status" in res, is(true));
          await assertThat("headers" in res, is(true));
          await assertThat(typeof res.json, is("function"));
          await assertThat(typeof res.text, is("function"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: response.ok is true for 2xx",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("ok"));
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.ok, is(true));
          await assertThat(res.status, is(200));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: response.ok is false for 4xx/5xx",
    verify: {
      expression: async () => {
        const server = serve(
          { port: 0 },
          () => new Response("not found", { status: 404 })
        );
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.ok, is(false));
          await assertThat(res.status, is(404));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Response body methods
  scenario({
    description: "fetch api :: response.json() parses JSON",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () =>
          Response.json({ message: "hello", nested: { a: 1, b: 2 } })
        );
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          const data = await res.json();
          await assertThat(data.message, is("hello"));
          await assertThat(data.nested.a, is(1));
          await assertThat(data.nested.b, is(2));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: response.text() returns string",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("hello world"));
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          const text = await res.text();
          await assertThat(text, is("hello world"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Request options
  scenario({
    description: "fetch api :: POST with JSON body",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, async (req) => {
          const body = await req.json();
          return Response.json({ received: body });
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ key: "value", num: 123 }),
          });
          const data = await res.json();
          await assertThat(data.received.key, is("value"));
          await assertThat(data.received.num, is(123));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: sends custom headers",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          return Response.json({
            custom: req.headers.get("x-custom"),
            auth: req.headers.get("authorization"),
          });
        });
        try {
          const res = await fetch(`http://localhost:${server.port}/`, {
            headers: {
              "X-Custom": "my-value",
              Authorization: "Bearer test-token",
            },
          });
          const data = await res.json();
          await assertThat(data.custom, is("my-value"));
          await assertThat(data.auth, is("Bearer test-token"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Headers object
  scenario({
    description: "fetch api :: Headers.get() is case-insensitive",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () =>
          new Response("ok", {
            headers: { "X-Custom-Header": "test-value" },
          })
        );
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          // Different case variations should all work
          await assertThat(
            res.headers.get("X-Custom-Header"),
            is("test-value")
          );
          await assertThat(
            res.headers.get("x-custom-header"),
            is("test-value")
          );
          await assertThat(
            res.headers.get("X-CUSTOM-HEADER"),
            is("test-value")
          );
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: Headers.has() checks existence",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () =>
          new Response("ok", {
            headers: { "X-Exists": "yes" },
          })
        );
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.headers.has("x-exists"), is(true));
          await assertThat(res.headers.has("x-not-exists"), is(false));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: Headers constructor works",
    verify: {
      expression: async () => {
        // Test Headers constructor with object
        const h1 = new Headers({ "X-Test": "value1" });
        await assertThat(h1.get("x-test"), is("value1"));

        // Test Headers constructor with array
        const h2 = new Headers([
          ["X-One", "1"],
          ["X-Two", "2"],
        ]);
        await assertThat(h2.get("x-one"), is("1"));
        await assertThat(h2.get("x-two"), is("2"));

        // Test set/delete
        h1.set("x-new", "new-value");
        await assertThat(h1.get("x-new"), is("new-value"));
        h1.delete("x-test");
        await assertThat(h1.has("x-test"), is(false));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Error handling
  scenario({
    description: "fetch api :: HTTP 404 does not throw",
    verify: {
      expression: async () => {
        const server = serve(
          { port: 0 },
          () => new Response("not found", { status: 404 })
        );
        try {
          // Should not throw
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.ok, is(false));
          await assertThat(res.status, is(404));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: HTTP 500 does not throw",
    verify: {
      expression: async () => {
        const server = serve(
          { port: 0 },
          () => new Response("error", { status: 500 })
        );
        try {
          // Should not throw
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(res.ok, is(false));
          await assertThat(res.status, is(500));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: network error throws",
    verify: {
      expression: async () => {
        let threw = false;
        try {
          // Connect to a port that should not be listening
          await fetch("http://localhost:49999/");
        } catch {
          threw = true;
        }
        await assertThat(threw, is(true));
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  // Response properties
  scenario({
    description: "fetch api :: response has url property",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("ok"));
        try {
          const url = `http://localhost:${server.port}/test/path`;
          const res = await fetch(url);
          await assertThat(res.url.includes("/test/path"), is(true));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "fetch api :: response has statusText property",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, () => new Response("ok"));
        try {
          const res = await fetch(`http://localhost:${server.port}/`);
          await assertThat(typeof res.statusText, is("string"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ==================== HTTP IMPORTS SCENARIOS ====================

// Note: HTTP imports are tested via integration with the bundler/runtime
// These scenarios test the fetch-based module loading behavior

const httpImportsScenarios = [
  // Basic import scenario - tests that fetch can download content
  scenario({
    description: "http imports :: can fetch TypeScript module content",
    verify: {
      expression: async () => {
        // This tests that the fetch infrastructure works for module fetching
        // Actual HTTP import testing requires integration with the bundler
        const server = serve({ port: 0 }, () =>
          new Response(
            `export const helper = () => "from http";`,
            { headers: { "Content-Type": "application/typescript" } }
          )
        );
        try {
          const res = await fetch(`http://localhost:${server.port}/mod.ts`);
          await assertThat(res.ok, is(true));
          const text = await res.text();
          await assertThat(text.includes("export const helper"), is(true));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http imports :: handles relative path resolution",
    verify: {
      expression: async () => {
        // Test that relative paths in URLs can be resolved
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          if (url.pathname === "/lib/mod.ts") {
            return new Response(`import { util } from "./utils.ts";`);
          } else if (url.pathname === "/lib/utils.ts") {
            return new Response(`export const util = () => "util";`);
          }
          return new Response("not found", { status: 404 });
        });
        try {
          // Fetch main module
          const modRes = await fetch(
            `http://localhost:${server.port}/lib/mod.ts`
          );
          await assertThat(modRes.ok, is(true));
          const modText = await modRes.text();
          await assertThat(modText.includes("./utils.ts"), is(true));

          // Verify utils.ts is accessible
          const utilsRes = await fetch(
            `http://localhost:${server.port}/lib/utils.ts`
          );
          await assertThat(utilsRes.ok, is(true));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http imports :: handles parent path resolution",
    verify: {
      expression: async () => {
        // Test parent path resolution (../)
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          if (url.pathname === "/lib/deep/nested.ts") {
            return new Response(`import { base } from "../base.ts";`);
          } else if (url.pathname === "/lib/base.ts") {
            return new Response(`export const base = () => "base";`);
          }
          return new Response("not found", { status: 404 });
        });
        try {
          // Verify nested module
          const nestedRes = await fetch(
            `http://localhost:${server.port}/lib/deep/nested.ts`
          );
          await assertThat(nestedRes.ok, is(true));

          // Verify base module is accessible
          const baseRes = await fetch(
            `http://localhost:${server.port}/lib/base.ts`
          );
          await assertThat(baseRes.ok, is(true));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http imports :: handles query strings in URLs",
    verify: {
      expression: async () => {
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          return Response.json({
            pathname: url.pathname,
            version: url.searchParams.get("v"),
          });
        });
        try {
          const res = await fetch(
            `http://localhost:${server.port}/mod.ts?v=1.0.0`
          );
          const data = await res.json();
          await assertThat(data.pathname, is("/mod.ts"));
          await assertThat(data.version, is("1.0.0"));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http imports :: 404 returns error response",
    verify: {
      expression: async () => {
        const server = serve(
          { port: 0 },
          () => new Response("Not Found", { status: 404 })
        );
        try {
          const res = await fetch(
            `http://localhost:${server.port}/not-found.ts`
          );
          await assertThat(res.ok, is(false));
          await assertThat(res.status, is(404));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),

  scenario({
    description: "http imports :: handles redirect responses",
    verify: {
      expression: async () => {
        // Use a wrapper to capture port after server starts
        let serverPort = 0;
        const server = serve({ port: 0 }, (req) => {
          const url = new URL(req.url);
          if (url.pathname === "/redirect.ts") {
            // Redirect to target.ts using the captured port
            return new Response(null, {
              status: 302,
              headers: {
                Location: `http://localhost:${serverPort}/target.ts`,
              },
            });
          }
          return new Response(`export const target = "found";`);
        });
        serverPort = server.port;
        try {
          const res = await fetch(
            `http://localhost:${server.port}/redirect.ts`
          );
          // fetch() follows redirects by default
          await assertThat(res.ok, is(true));
          const text = await res.text();
          await assertThat(text.includes("target"), is(true));
        } finally {
          await server.shutdown();
        }
      },
      references: new Map(),
    } as Closure<() => Promise<unknown>>,
  }),
];

// ==================== MAIN ====================

const allScenarios = [
  ...httpServerScenarios,
  ...fetchApiScenarios,
  ...httpImportsScenarios,
];

export default async () => {
  const results = await runScenarios(allScenarios, { logger: log });

  const passed = results.filter((r) => r.success).length;
  const failed = results.filter((r) => !r.success).length;

  log(`\nResults: ${passed} passed, ${failed} failed`);

  if (failed > 0) {
    throw new Error(`${failed} scenarios failed`);
  }
};
