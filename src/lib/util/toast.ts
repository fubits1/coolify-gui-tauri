import { toast as sonner } from "svelte-sonner";

/**
 * Thin typed wrapper around svelte-sonner's `toast` API.
 *
 * Persistence policy:
 * - **success / info** → 10s, auto-dismiss. Acknowledgement only.
 * - **warning / error / loading** → sticky until user dismisses. Actionable.
 *
 * Every variant gets a close button so a sticky toast can't trap the UI.
 */
const PERSIST = Number.POSITIVE_INFINITY;
const TRANSIENT = 10_000;
const STICKY = { duration: PERSIST, closeButton: true } as const;
const FADING = { duration: TRANSIENT, closeButton: true } as const;

export const toast = {
  success: (msg: string, description?: string) =>
    sonner.success(msg, { description, ...FADING }),
  error: (msg: string, description?: string) =>
    sonner.error(msg, { description, ...STICKY }),
  info: (msg: string, description?: string) =>
    sonner.info(msg, { description, ...FADING }),
  /** Sticky — for "actionable" info that the user must see + ack
   *  (e.g. "15 images have a newer version available"). */
  warning: (msg: string, description?: string) =>
    sonner.warning(msg, { description, ...STICKY }),
  /** Loading toasts stay sticky — `promise()` swaps them on settle. */
  loading: (msg: string) => sonner.loading(msg, STICKY),
  promise: <T>(
    p: Promise<T>,
    opts: {
      loading: string;
      success: string | ((data: T) => string);
      error: string | ((err: unknown) => string);
    },
  ) =>
    sonner.promise(p, {
      ...opts,
      duration: PERSIST,
      closeButton: true,
    }),
  dismiss: (id?: string | number) => sonner.dismiss(id),
};
