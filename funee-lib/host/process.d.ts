declare module "host://process" {
  export type Signal =
    | "SIGTERM"
    | "SIGKILL"
    | "SIGINT"
    | "SIGHUP"
    | "SIGQUIT";

  export interface SpawnOptions {
    cmd: string[];
    cwd?: string;
    env?: Record<string, string>;
    inheritEnv?: boolean;
    stdin?: "piped" | "inherit" | "null";
    stdout?: "piped" | "inherit" | "null";
    stderr?: "piped" | "inherit" | "null";
  }

  export interface ProcessStatus {
    success: boolean;
    code: number | null;
    signal: Signal | null;
  }

  export interface CommandOutput {
    status: ProcessStatus;
    stdout: Uint8Array;
    stderr: Uint8Array;
    stdoutText(): string;
    stderrText(): string;
  }

  export interface Process {
    readonly pid: number;
    readonly status: Promise<ProcessStatus>;
    readonly stdout: AsyncIterable<string | Uint8Array>;
    readonly stderr: AsyncIterable<string | Uint8Array>;
    kill(signal?: Signal): void;
    completion(): Promise<{ status: number; signal: Signal | null }>;
    output(): Promise<CommandOutput>;
    writeInput(data: string | Uint8Array): Promise<void>;
    [Symbol.asyncDispose](): Promise<void>;
  }

  export function env(name: string): string | undefined;
  export function spawn(command: string, args?: string[]): Promise<CommandOutput>;
  export function spawn(options: SpawnOptions): Process;
}
