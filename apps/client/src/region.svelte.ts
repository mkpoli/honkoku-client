import {
  isTauri,
  regionCached,
  regionRefresh,
  type ReadRegion,
} from "@honkoku/client-api/invoke";
import { untrack } from "svelte";
import { errorMessage } from "./lib";

const cache = new Map<string, unknown>();

/** A region keeps its cached value while a fresh request is in flight. */
export class Region<T> {
  value = $state<T>();
  pending = $state(false);
  slow = $state(false);
  error = $state("");
  private generation = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private key = "";
  private request: (() => Promise<T>) | undefined;
  private native: ReadRegion | undefined;
  private accept: ((value: T) => void) | undefined;

  load(
    key: string,
    request: () => Promise<T>,
    accept?: (value: T) => void,
    native?: ReadRegion,
  ) {
    return untrack(() => {
      this.cancel();
      const generation = this.generation;
      this.key = key;
      this.request = request;
      this.accept = accept;
      this.native = native;
      this.value = cache.get(key) as T | undefined;
      if (this.value !== undefined) accept?.(this.value);
      this.pending = true;
      this.error = "";
      this.slow = false;
      this.timer = setTimeout(() => {
        if (generation === this.generation) this.slow = true;
      }, 15_000);
      let fresh = false;
      if (native && isTauri() && this.value === undefined) {
        void regionCached<T>(native)
          .then((value) => {
            if (value !== null && !fresh && generation === this.generation) {
              cache.set(key, value);
              this.value = value;
              accept?.(value);
            }
          })
          .catch(() => {});
      }
      return (native && isTauri() ? regionRefresh<T>(native) : request())
        .then((value) => {
          fresh = true;
          if (generation !== this.generation) return;
          cache.set(key, value);
          this.value = value;
          accept?.(value);
        })
        .catch((error) => {
          if (generation === this.generation) this.error = errorMessage(error);
        })
        .finally(() => {
          if (generation === this.generation) {
            clearTimeout(this.timer);
            this.pending = false;
            this.slow = false;
          }
        });
    });
  }

  retry = () => {
    if (this.request)
      void this.load(this.key, this.request, this.accept, this.native);
  };

  cancel() {
    this.generation++;
    clearTimeout(this.timer);
    this.pending = false;
    this.slow = false;
  }
}
