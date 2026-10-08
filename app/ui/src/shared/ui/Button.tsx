import type { ButtonHTMLAttributes } from "react";
import styles from "./controls.module.css";

type Variant = "default" | "primary" | "ghost" | "danger";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant };

export function Button({ variant = "default", className, type = "button", ...rest }: ButtonProps) {
  const variantClass = variant === "default" ? "" : styles[variant];
  return <button type={type} className={[styles.button, variantClass, className].filter(Boolean).join(" ")} {...rest} />;
}
