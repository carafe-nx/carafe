import type { RegistryChange } from "@/shared/api/bindings/RegistryChange";
import type { RegistryEntry } from "@/shared/api/bindings/RegistryEntry";
import type { RegistryKind } from "@/shared/api/bindings/RegistryKind";
import type { RegistryRoot } from "@/shared/api/bindings/RegistryRoot";
import type { RegistryValue } from "@/shared/api/bindings/RegistryValue";

export type EntryType =
  | "string"
  | "expandString"
  | "multiString"
  | "dword"
  | "qword"
  | "binary"
  | "other"
  | "deleteValue"
  | "deleteKey";

export const editableTypes: readonly EntryType[] = [
  "string",
  "expandString",
  "multiString",
  "dword",
  "qword",
  "binary",
  "deleteValue",
  "deleteKey",
];

export const menuKinds: readonly RegistryKind[] = ["dword", "string", "qword", "expandString"];

export const rootShort: Record<RegistryRoot, string> = {
  currentUser: "HKCU",
  localMachine: "HKLM",
};

export function entryType(change: RegistryChange): EntryType {
  if (change.kind === "set") {
    return change.value.type;
  }
  return change.kind;
}

function hexBytes(bytes: number[]): string {
  return bytes.map((byte) => byte.toString(16).padStart(2, "0")).join(" ");
}

export function valueText(value: RegistryValue): string {
  switch (value.type) {
    case "string":
    case "expandString":
      return value.text;
    case "multiString":
      return value.items.join("; ");
    case "dword":
      return String(value.number);
    case "qword":
      return value.number;
    case "binary":
    case "other":
      return hexBytes(value.bytes);
  }
}

export function changeText(change: RegistryChange): string {
  return change.kind === "set" ? valueText(change.value) : "";
}

function parseNumber(text: string, max: bigint): bigint | null {
  const trimmed = text.trim();
  const valid = /^0x[0-9a-f]+$/i.test(trimmed) || /^[0-9]+$/.test(trimmed);
  if (!valid) {
    return null;
  }
  const number = BigInt(trimmed);
  return number <= max ? number : null;
}

function parseBytes(text: string): number[] | null {
  const parts = text
    .split(/[\s,]+/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
  const bytes: number[] = [];
  for (const part of parts) {
    if (!/^[0-9a-f]{1,2}$/i.test(part)) {
      return null;
    }
    bytes.push(Number.parseInt(part, 16));
  }
  return bytes;
}

export function parseChange(type: EntryType, text: string, kind: number): RegistryChange | null {
  switch (type) {
    case "deleteValue":
    case "deleteKey":
      return { kind: type };
    case "string":
    case "expandString":
      return { kind: "set", value: { type, text } };
    case "multiString":
      return {
        kind: "set",
        value: {
          type,
          items: text
            .split(";")
            .map((item) => item.trim())
            .filter((item) => item.length > 0),
        },
      };
    case "dword": {
      const number = parseNumber(text, 0xffffffffn);
      return number === null ? null : { kind: "set", value: { type, number: Number(number) } };
    }
    case "qword": {
      const number = parseNumber(text, 0xffffffffffffffffn);
      return number === null ? null : { kind: "set", value: { type, number: number.toString() } };
    }
    case "binary": {
      const bytes = parseBytes(text);
      return bytes === null ? null : { kind: "set", value: { type, bytes } };
    }
    case "other": {
      const bytes = parseBytes(text);
      return bytes === null ? null : { kind: "set", value: { type, kind, bytes } };
    }
  }
}

export function kindValid(kind: RegistryKind, text: string): boolean {
  if (kind === "dword") {
    return parseNumber(text, 0xffffffffn) !== null;
  }
  if (kind === "qword") {
    return parseNumber(text, 0xffffffffffffffffn) !== null;
  }
  return !/[\t\r\n]/.test(text);
}

export function sameValue(a: RegistryEntry, b: RegistryEntry): boolean {
  const path = (entry: RegistryEntry) => entry.path.replace(/^\\+|\\+$/g, "").toLowerCase();
  const target = (entry: RegistryEntry) => (entry.change.kind === "deleteKey" ? "key" : "value");
  return (
    a.root === b.root &&
    path(a) === path(b) &&
    a.name.toLowerCase() === b.name.toLowerCase() &&
    target(a) === target(b)
  );
}

export function mergeEntries(current: RegistryEntry[], added: RegistryEntry[]): RegistryEntry[] {
  const kept = current.filter((entry) => !added.some((next) => sameValue(entry, next)));
  return [...kept, ...added];
}
