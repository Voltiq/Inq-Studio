import type { ReactNode } from "react";
import {
  CircleAlert,
  CircleCheck,
  Info,
  TriangleAlert,
  type LucideIcon,
} from "lucide-react";
import { cx } from "../../lib/utils";

export function Spinner({ size = 16, label }: { size?: number; label?: string }) {
  return (
    <svg
      className="spinner"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <circle cx="8" cy="8" r="6.5" fill="none" strokeWidth="1.5" />
    </svg>
  );
}

export type Role = "default" | "success" | "danger" | "warning" | "information" | "discovery";

export function Lozenge({
  appearance = "default",
  icon: Icon,
  children,
  title,
}: {
  appearance?: Role;
  icon?: LucideIcon;
  children: ReactNode;
  title?: string;
}) {
  return (
    <span className={cx("lozenge", `lozenge-${appearance}`)} data-tooltip={title}>
      {Icon && <Icon size={12} aria-hidden="true" />}
      {children}
    </span>
  );
}

export function Badge({
  children,
  appearance = "default",
}: {
  children: ReactNode;
  appearance?: "default" | "primary" | "danger";
}) {
  return <span className={cx("badge", `badge-${appearance}`)}>{children}</span>;
}

export function ProgressBar({
  value,
  appearance = "default",
  indeterminate,
  label,
}: {
  value?: number;
  appearance?: "default" | "success" | "danger" | "muted";
  indeterminate?: boolean;
  label?: string;
}) {
  const pct = Math.max(0, Math.min(1, value ?? 0)) * 100;
  return (
    <div
      className={cx("progress", `progress-${appearance}`, indeterminate && "is-indeterminate")}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : Math.round(pct)}
    >
      <div
        className="progress-fill"
        style={indeterminate ? undefined : { transform: `scaleX(${pct / 100})` }}
      />
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="kbd">{children}</kbd>;
}

/** Renders "Ctrl+Shift+N" as separate key caps. */
export function Shortcut({ keys }: { keys: string }) {
  const parts = keys === "+" ? ["+"] : keys.split("+");
  return (
    <span className="shortcut">
      {parts.map((k, i) => (
        <Kbd key={i}>{k}</Kbd>
      ))}
    </span>
  );
}

const MESSAGE_ICONS: Record<Exclude<Role, "default" | "discovery">, LucideIcon> = {
  information: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  danger: CircleAlert,
};

export function SectionMessage({
  appearance = "information",
  title,
  children,
  actions,
}: {
  appearance?: "information" | "success" | "warning" | "danger";
  title?: string;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  const Icon = MESSAGE_ICONS[appearance];
  return (
    <div
      className={cx("section-message", `section-message-${appearance}`)}
      role={appearance === "danger" ? "alert" : "status"}
    >
      <Icon size={16} className="section-message-icon" aria-hidden="true" />
      <div className="section-message-content">
        {title && <p className="section-message-title">{title}</p>}
        {children && <div className="section-message-body">{children}</div>}
        {actions && <div className="section-message-actions">{actions}</div>}
      </div>
    </div>
  );
}

export function EmptyState({
  icon: Icon,
  title,
  children,
  actions,
  compact,
}: {
  icon: LucideIcon;
  title: string;
  children?: ReactNode;
  actions?: ReactNode;
  compact?: boolean;
}) {
  return (
    <div className={cx("empty-state", compact && "is-compact")}>
      <span className="icon-tile" aria-hidden="true">
        <Icon size={16} />
      </span>
      <h3 className="empty-state-title">{title}</h3>
      {children && <p className="empty-state-body">{children}</p>}
      {actions && <div className="empty-state-actions">{actions}</div>}
    </div>
  );
}

export function StatusDot({ status }: { status: "online" | "busy" | "offline" | "warning" }) {
  return <span className={cx("status-dot", `status-dot-${status}`)} aria-hidden="true" />;
}
