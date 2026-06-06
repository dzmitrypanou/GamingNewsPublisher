export type DialogVariant = "info" | "success" | "error";

export interface AlertOptions {
  title?: string;
  variant?: DialogVariant;
}

export interface ConfirmOptions {
  title?: string;
  confirmText?: string;
  cancelText?: string;
  destructive?: boolean;
  variant?: DialogVariant;
}

export type DialogRequest =
  | {
      kind: "alert";
      message: string;
      title: string;
      variant: DialogVariant;
      resolve: () => void;
    }
  | {
      kind: "confirm";
      message: string;
      title: string;
      confirmText: string;
      cancelText: string;
      destructive: boolean;
      variant: DialogVariant;
      resolve: (value: boolean) => void;
    };

type DialogListener = (request: DialogRequest | null) => void;

let listener: DialogListener | null = null;

export function subscribeDialog(next: DialogListener) {
  listener = next;
  return () => {
    if (listener === next) {
      listener = null;
    }
  };
}

function showAlert(message: string, options: AlertOptions = {}) {
  return new Promise<void>((resolve) => {
    listener?.({
      kind: "alert",
      message,
      title: options.title ?? "Уведомление",
      variant: options.variant ?? "info",
      resolve,
    });
  });
}

function showConfirm(message: string, options: ConfirmOptions = {}) {
  return new Promise<boolean>((resolve) => {
    listener?.({
      kind: "confirm",
      message,
      title: options.title ?? "Подтверждение",
      confirmText: options.confirmText ?? "OK",
      cancelText: options.cancelText ?? "Отмена",
      destructive: options.destructive ?? false,
      variant: options.variant ?? "info",
      resolve,
    });
  });
}

export const dialog = {
  alert: showAlert,
  confirm: showConfirm,
};
