import type { ButtonHTMLAttributes, ReactNode } from "react";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual emphasis. */
  variant?: "primary" | "secondary" | "ghost" | "danger";
  children: ReactNode;
}

const VARIANT_CLASSES: Record<NonNullable<ButtonProps["variant"]>, string> = {
  primary: "bg-accent text-on-accent hover:bg-accent-strong",
  secondary:
    "bg-surface-muted text-content-primary border border-border-default hover:bg-surface-raised",
  ghost: "bg-transparent text-content-secondary hover:bg-surface-muted",
  danger: "bg-status-error text-content-inverse hover:opacity-90",
};

/**
 * Accessible button primitive styled with OpenConKit design tokens.
 */
export function Button({ variant = "primary", children, type, className, ...rest }: ButtonProps) {
  return (
    <button
      // Native buttons default to type="submit" inside forms; be explicit.
      type={type ?? "button"}
      className={`inline-flex items-center justify-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors focus-visible:focus-ring disabled:cursor-not-allowed disabled:opacity-50 ${VARIANT_CLASSES[variant]} ${className ?? ""}`}
      {...rest}
    >
      {children}
    </button>
  );
}
