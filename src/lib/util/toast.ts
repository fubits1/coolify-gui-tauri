import { toast as sonner } from "svelte-sonner";

/**
 * Thin typed wrapper around svelte-sonner's `toast` API.
 *
 * Errors persist until the user dismisses — they carry actionable info that
 * vanishing-after-4s defeats. Success/info follow the default sonner timeout.
 */
const ERROR_DURATION = Number.POSITIVE_INFINITY;

export const toast = {
  success: (msg: string, description?: string) =>
    sonner.success(msg, { description }),
  error: (msg: string, description?: string) =>
    sonner.error(msg, {
      description,
      duration: ERROR_DURATION,
      closeButton: true,
    }),
  info: (msg: string, description?: string) =>
    sonner.info(msg, { description }),
  loading: (msg: string) => sonner.loading(msg),
  promise: <T>(
    p: Promise<T>,
    opts: {
      loading: string;
      success: string | ((data: T) => string);
      error: string | ((err: unknown) => string);
    },
  ) => sonner.promise(p, opts),
  dismiss: (id?: string | number) => sonner.dismiss(id),
};
