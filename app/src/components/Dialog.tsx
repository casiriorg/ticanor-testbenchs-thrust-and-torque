import type { ReactNode } from "react";

interface DialogProps {
  title: string;
  children: ReactNode;
  /** Buttons, right aligned. */
  actions?: ReactNode;
  onClose?: () => void;
  wide?: boolean;
}

/** Modal dialog. Clicking the backdrop closes it only when `onClose` is given. */
export function Dialog({ title, children, actions, onClose, wide }: DialogProps) {
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose?.()}>
      <div className={`dialog${wide ? " wide" : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <h2>{title}</h2>
        <div className="dialog-body">{children}</div>
        {actions && <div className="dialog-actions">{actions}</div>}
      </div>
    </div>
  );
}
