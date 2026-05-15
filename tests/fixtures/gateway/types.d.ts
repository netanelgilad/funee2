/// <reference path="../../../funee-lib/host/process.d.ts" />

declare module "host://http" {
  export function fetch(
    input: string | URL | Request,
    init?: RequestInit,
  ): Promise<Response>;
}

declare module "host://http/server" {
  export type RequestHandler = (
    request: Request,
  ) => Response | Promise<Response>;

  export interface ServeOptions {
    port: number;
    hostname?: string;
    onListen?: (info: { port: number; hostname: string }) => void;
    onError?: (error: Error) => Response | Promise<Response>;
  }

  export interface Server {
    readonly port: number;
    readonly hostname: string;
    shutdown(): Promise<void>;
    [Symbol.asyncDispose](): Promise<void>;
  }

  export function serve(options: ServeOptions, handler: RequestHandler): Server;
  export function createResponse(
    body?: string | null,
    init?: ResponseInit,
  ): Response;
  export function createJsonResponse(
    data: unknown,
    init?: ResponseInit,
  ): Response;
}
