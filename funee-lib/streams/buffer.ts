export type BufferedStreamSnapshot = {
  text(): string;
};

export const buffer = function(
  iterable: AsyncIterable<string | Uint8Array>
): BufferedStreamSnapshot {
  const decoder = new TextDecoder();
  const parts: string[] = [];

  (async () => {
    for await (const chunk of iterable) {
      if (typeof chunk === "string") {
        parts.push(chunk);
      } else {
        parts.push(decoder.decode(chunk, { stream: true }));
      }
    }
    parts.push(decoder.decode());
  })();

  return {
    text() {
      return parts.join("");
    },
  };
};
