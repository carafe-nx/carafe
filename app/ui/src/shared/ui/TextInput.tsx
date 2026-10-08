import type { InputHTMLAttributes } from "react";
import styles from "./controls.module.css";

export function TextInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={[styles.input, className].filter(Boolean).join(" ")} {...rest} />;
}
