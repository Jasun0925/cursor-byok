import { useId, type ReactNode } from "react";
import { Modal } from "./Modal";

export function ConfirmDialog({ id, open, title, children, busy, wide, returnFocus, cancelLabel = t("取消"), confirmLabel = t("确认"), onCancel, onConfirm }: {
  id?: string;
  open: boolean;
  title: string;
  children: ReactNode;
  busy?: boolean;
  wide?: boolean;
  returnFocus?: () => HTMLElement | null;
  cancelLabel?: string;
  confirmLabel?: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const contentId = useId();
  return <Modal
    id={id}
    open={open}
    title={title}
    busy={busy}
    wide={wide}
    role="alertdialog"
    ariaDescribedBy={contentId}
    initialFocus="submit"
    returnFocus={returnFocus}
    closeLabel={cancelLabel}
    submitLabel={confirmLabel}
    onClose={onCancel}
    onSubmit={onConfirm}
  >
    <div id={contentId}>{children}</div>
  </Modal>;
}
