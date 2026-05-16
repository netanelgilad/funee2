/**
 * Host module type declarations for funee's `host://` import scheme.
 *
 * Reference this file from TypeScript entrypoints that import host modules:
 * `/// <reference path=".../funee-lib/host/index.d.ts" />`
 */

/// <reference path="./process.d.ts" />

declare module "host://console" {
  export function log(...args: unknown[]): void;
  export function debug(...args: unknown[]): void;
}

declare module "host://crypto" {
  export function randomBytes(length: number): Uint8Array;
}

declare module "host://fs" {
  export interface FileStats {
    size: number;
    is_file: boolean;
    is_directory: boolean;
    modified_ms: number;
  }

  export function readFile(path: string): string;
  export function readFileBinary(path: string): string;
  export function writeFile(path: string, content: string): string;
  export function removeFile(path: string): string;
  export function writeFileBinary(path: string, contentBase64: string): string;
  export function isFile(path: string): boolean;
  export function exists(path: string): boolean;
  export function lstat(path: string): string;
  export function mkdir(path: string, recursive?: boolean): void;
  export function readdir(path: string): string;
  export function tmpdir(): string;
}

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

declare module "host://time" {
  export function setTimeout(callback: () => void, ms: number): number;
  export function clearTimeout(id: number): void;
  export function setInterval(callback: () => void, ms: number): number;
  export function clearInterval(id: number): void;
}

declare module "host://watch" {
  export function watchStart(path: string, recursive: boolean): string;
  export function watchPoll(watcherId: number): string;
  export function watchStop(watcherId: number): void;
}
