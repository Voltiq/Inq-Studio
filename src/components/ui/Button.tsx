import { forwardRef, type ButtonHTMLAttributes } from "react";
import type { LucideIcon } from "lucide-react";
import { cx } from "../../lib/utils";
import { Spinner } from "./Feedback";

type Appearance = "default" | "primary" | "subtle" | "danger" | "warning";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  appearance?: Appearance;
  spacing?: "default" | "compact";
  iconBefore?: LucideIcon;
  iconAfter?: LucideIcon;
  loading?: boolean;
  selected?: boolean;
  /** Shown in the tooltip next to the label. */
  shortcut?: string;
  tooltip?: string;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    appearance = "default",
    spacing = "default",
    iconBefore: Before,
    iconAfter: After,
    loading,
    selected,
    shortcut,
    tooltip,
    className,
    children,
    disabled,
    type = "button",
    ...rest
  },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={cx(
        "btn",
        `btn-${appearance}`,
        spacing === "compact" && "btn-compact",
        selected && "is-selected",
        loading && "is-loading",
        className,
      )}
      disabled={disabled || loading}
      aria-pressed={selected}
      data-tooltip={tooltip}
      data-tooltip-kbd={shortcut}
      {...rest}
    >
      {loading ? (
        <Spinner size={16} />
      ) : (
        Before && <Before size={16} aria-hidden="true" />
      )}
      {children != null && <span className="btn-label">{children}</span>}
      {After && <After size={16} aria-hidden="true" />}
    </button>
  );
});

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: LucideIcon;
  /** Accessible name, also used as the tooltip. */
  label: string;
  appearance?: "subtle" | "default" | "primary";
  spacing?: "default" | "compact";
  shortcut?: string;
  selected?: boolean;
  iconClassName?: string;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton(
    {
      icon: Icon,
      label,
      appearance = "subtle",
      spacing = "default",
      shortcut,
      selected,
      className,
      iconClassName,
      type = "button",
      ...rest
    },
    ref,
  ) {
    return (
      <button
        ref={ref}
        type={type}
        className={cx(
          "icon-btn",
          `icon-btn-${appearance}`,
          spacing === "compact" && "icon-btn-compact",
          selected && "is-selected",
          className,
        )}
        aria-label={label}
        aria-pressed={selected}
        data-tooltip={label}
        data-tooltip-kbd={shortcut}
        {...rest}
      >
        <Icon size={16} aria-hidden="true" className={iconClassName} />
      </button>
    );
  },
);
