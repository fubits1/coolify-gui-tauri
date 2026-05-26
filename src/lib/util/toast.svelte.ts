import { toast as sonner } from "svelte-sonner";

/**
 * Thin typed wrapper around svelte-sonner's `toast` API.
 *
 * Persistence policy:
 * - **success / info** → 10s, auto-dismiss. Acknowledgement only.
 * - **warning / error / loading** → sticky until user dismisses. Actionable.
 *
 * Every variant gets a close button so a sticky toast can't trap the UI.
 *
 * Active-toast counter (`activeCount`) lets the UI render a "Dismiss all"
 * button while at least one sticky toast is on screen — sonner doesn't
 * expose its internal stack count reactively, so we keep our own.
 */
const PERSIST = Number.POSITIVE_INFINITY;
const TRANSIENT = 10_000;

let activeCount = $state(0);
export function toastActiveCount(): number {
  return activeCount;
}

function track<T extends number | string>(id: T): T {
  activeCount += 1;
  return id;
}

const STICKY = {
  duration: PERSIST,
  closeButton: true,
  onDismiss: () => {
    activeCount = Math.max(0, activeCount - 1);
  },
  onAutoClose: () => {
    activeCount = Math.max(0, activeCount - 1);
  },
} as const;
const FADING = {
  duration: TRANSIENT,
  closeButton: true,
  onDismiss: () => {
    activeCount = Math.max(0, activeCount - 1);
  },
  onAutoClose: () => {
    activeCount = Math.max(0, activeCount - 1);
  },
} as const;

export const toast = {
  success: (msg: string, description?: string) =>
    track(sonner.success(msg, { description, ...FADING })),
  error: (msg: string, description?: string) =>
    track(sonner.error(msg, { description, ...STICKY })),
  info: (msg: string, description?: string) =>
    track(sonner.info(msg, { description, ...FADING })),
  warning: (msg: string, description?: string) =>
    track(sonner.warning(msg, { description, ...STICKY })),
  loading: (msg: string) => track(sonner.loading(msg, STICKY)),
  promise: <T>(
    p: Promise<T>,
    opts: {
      loading: string;
      success: string | ((data: T) => string);
      error: string | ((err: unknown) => string);
    },
  ) => {
    activeCount += 1;
    return sonner.promise(p, {
      ...opts,
      duration: PERSIST,
      closeButton: true,
      onDismiss: () => {
        activeCount = Math.max(0, activeCount - 1);
      },
      onAutoClose: () => {
        activeCount = Math.max(0, activeCount - 1);
      },
    });
  },
  dismiss: (id?: string | number) => {
    if (id === undefined) {
      activeCount = 0;
      // Some sonner builds branch on `arguments.length` for the
      // dismiss-all path; passing `undefined` is treated as "dismiss
      // toast with id=undefined" → no-op. Call WITHOUT args here.
      sonner.dismiss();
    } else {
      activeCount = Math.max(0, activeCount - 1);
      sonner.dismiss(id);
    }
  },
};
