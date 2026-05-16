export type AsyncDisposableResource = {
  [Symbol.asyncDispose](): Promise<void>;
};

export const composeAsyncDisposables = <T extends object>(
  value: T,
  disposables: Array<AsyncDisposableResource | undefined>,
): T & AsyncDisposableResource => {
  const disposeValue = Symbol.asyncDispose in value
    ? (value as AsyncDisposableResource)[Symbol.asyncDispose].bind(value)
    : undefined;

  return Object.assign(value, {
    async [Symbol.asyncDispose]() {
      await disposeValue?.();

      for (const disposable of disposables) {
        await disposable?.[Symbol.asyncDispose]();
      }
    },
  });
};
