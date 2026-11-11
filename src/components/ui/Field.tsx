import {
  forwardRef,
  useId,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { Check, CircleAlert, Minus, X, type LucideIcon } from "lucide-react";
import { cx } from "../../lib/utils";

interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: string;
  helper?: ReactNode;
  error?: string | null;
  icon?: LucideIcon;
  after?: ReactNode;
  compact?: boolean;
  mono?: boolean;
  containerClassName?: string;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  {
    label,
    helper,
    error,
    icon: Icon,
    after,
    compact,
    mono,
    id,
    className,
    containerClassName,
    ...rest
  },
  ref,
) {
  const autoId = useId();
  const inputId = id ?? autoId;
  const helperId = `${inputId}-helper`;
  const describedBy = error || helper ? helperId : undefined;
  return (
    <div className={cx("field", containerClassName)}>
      {label && (
        <label className="field-label" htmlFor={inputId}>
          {label}
        </label>
      )}
      <div
        className={cx(
          "textfield",
          compact && "is-compact",
          error && "is-invalid",
          rest.disabled && "is-disabled",
          className,
        )}
      >
        {Icon && <Icon size={16} className="textfield-icon" aria-hidden="true" />}
        <input
          ref={ref}
          id={inputId}
          className={cx("textfield-input", mono && "is-mono")}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          spellCheck={false}
          autoComplete="off"
          {...rest}
        />
        {after}
      </div>
      {error ? (
        <p className="field-message is-error" id={helperId}>
          <CircleAlert size={12} aria-hidden="true" />
          {error}
        </p>
      ) : (
        helper && (
          <p className="field-message" id={helperId}>
            {helper}
          </p>
        )
      )}
    </div>
  );
});

export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  description?: ReactNode;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className={cx("toggle-row", disabled && "is-disabled")}>
      <label htmlFor={id} className="toggle-text">
        <span className="toggle-label">{label}</span>
        {description && <span className="toggle-description">{description}</span>}
      </label>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        className={cx("toggle", checked && "is-checked")}
        onClick={() => onChange(!checked)}
      >
        <Check size={10} className="toggle-glyph toggle-on" aria-hidden="true" strokeWidth={3} />
        <X size={10} className="toggle-glyph toggle-off" aria-hidden="true" strokeWidth={3} />
        <span className="toggle-thumb" />
      </button>
    </div>
  );
}

export function Checkbox({
  checked,
  onChange,
  label,
  disabled,
  indeterminate,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: ReactNode;
  disabled?: boolean;
  indeterminate?: boolean;
}) {
  return (
    <label className={cx("checkbox", disabled && "is-disabled")}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        ref={(el) => {
          if (el) el.indeterminate = !!indeterminate;
        }}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="checkbox-box" aria-hidden="true">
        {checked ? <Check size={12} strokeWidth={3} /> : indeterminate && <Minus size={12} strokeWidth={3} />}
      </span>
      <span className="checkbox-label">{label}</span>
    </label>
  );
}

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: LucideIcon;
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: SegmentOption<T>[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => {
        const Icon = o.icon;
        const active = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            className={cx("segment", active && "is-selected")}
            onClick={() => onChange(o.value)}
            onKeyDown={(e) => {
              const i = options.findIndex((x) => x.value === value);
              if (e.key === "ArrowRight" || e.key === "ArrowDown") {
                e.preventDefault();
                onChange(options[(i + 1) % options.length].value);
              } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
                e.preventDefault();
                onChange(options[(i - 1 + options.length) % options.length].value);
              }
            }}
            tabIndex={active ? 0 : -1}
          >
            {Icon && <Icon size={16} aria-hidden="true" />}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export { Select } from "./Select";

export function RadioList<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string; description?: string }[];
  onChange: (value: T) => void;
  label: string;
}) {
  const name = useId();
  return (
    <fieldset className="radio-list">
      <legend className="field-label">{label}</legend>
      {options.map((o) => (
        <label key={o.value} className={cx("radio", o.value === value && "is-checked")}>
          <input
            type="radio"
            name={name}
            value={o.value}
            checked={o.value === value}
            onChange={() => onChange(o.value)}
          />
          <span className="radio-dot" aria-hidden="true" />
          <span className="radio-text">
            <span className="radio-label">{o.label}</span>
            {o.description && <span className="radio-description">{o.description}</span>}
          </span>
        </label>
      ))}
    </fieldset>
  );
}
