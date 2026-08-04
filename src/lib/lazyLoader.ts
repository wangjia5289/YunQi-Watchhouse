export type AsyncLoader<T> = () => Promise<T>;

export function createRetryableLoader<T>(loader: AsyncLoader<T>): AsyncLoader<T> {
  let pending: Promise<T> | null = null;

  return () => {
    if (pending === null) {
      pending = loader().catch((error) => {
        pending = null;
        throw error;
      });
    }
    return pending;
  };
}

export function preloadSilently<T>(loader: AsyncLoader<T>): void {
  void loader().catch(() => {
    // Navigation can retry a transient chunk failure when the page is requested.
  });
}
