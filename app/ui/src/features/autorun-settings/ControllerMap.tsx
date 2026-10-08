import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Binding } from "@/shared/api/bindings/Binding";
import type { SwitchButton } from "@/shared/api/bindings/SwitchButton";
import { BindingPopover } from "./BindingPopover";
import { DEVICE_VIEWBOX, type GlyphName, MAP_ROWS, type MapRow, ROW_OF_BUTTON } from "./controllerLayout";
import styles from "./ControllerMap.module.css";
import { shortChordLabel } from "./keyNames";
import { SwitchDevice } from "./SwitchDevice";

type ControllerMapProps = {
  bindings: readonly Binding[];
  pad: boolean;
  onChange: (button: SwitchButton, codes: number[]) => void;
};

type Leader = { row: string; d: string; ax: number; ay: number; rx: number; ry: number };
type Editing = { button: SwitchButton; left: number; top: number };

const WIDE_MIN = 900;
const NARROW_MIN = 560;

type Layout = "wide" | "narrow" | "single";

function layoutFor(width: number): Layout {
  if (width >= WIDE_MIN) {
    return "wide";
  }
  return width >= NARROW_MIN ? "narrow" : "single";
}
const [VIEW_X, VIEW_Y, VIEW_W] = DEVICE_VIEWBOX.split(" ").map(Number) as [number, number, number, number];

function Glyph({ name }: { name: GlyphName }) {
  switch (name) {
    case "trigger":
      return (
        <svg viewBox="0 0 28 28" className={styles.glyph} aria-hidden="true">
          <rect x="5" y="9" width="18" height="10" rx="5" />
        </svg>
      );
    case "shoulder":
      return (
        <svg viewBox="0 0 28 28" className={styles.glyph} aria-hidden="true">
          <path d="M5 20a11 11 0 0 1 11-10h7" strokeWidth="3" strokeLinecap="round" />
        </svg>
      );
    case "minus":
      return (
        <svg viewBox="0 0 28 28" className={styles.glyph} aria-hidden="true">
          <path d="M8 14h12" strokeWidth="2.6" strokeLinecap="round" />
        </svg>
      );
    case "plus":
      return (
        <svg viewBox="0 0 28 28" className={styles.glyph} aria-hidden="true">
          <path d="M8 14h12M14 8v12" strokeWidth="2.6" strokeLinecap="round" />
        </svg>
      );
    case "stick":
      return (
        <svg viewBox="0 0 28 28" className={styles.glyph} aria-hidden="true">
          <circle cx="14" cy="14" r="10" />
          <circle cx="14" cy="14" r="5.5" className={styles.glyphFill} />
        </svg>
      );
    case "dpad":
    case "abxy":
      return (
        <svg viewBox="0 0 28 28" className={styles.glyph} aria-hidden="true">
          {[
            [14, 6.5],
            [14, 21.5],
            [6.5, 14],
            [21.5, 14],
          ].map(([cx, cy]) => (
            <circle key={`${cx}-${cy}`} cx={cx} cy={cy} r="3.4" className={name === "abxy" ? styles.glyphFill : undefined} />
          ))}
        </svg>
      );
  }
}

export function ControllerMap({ bindings, pad, onChange }: ControllerMapProps) {
  const { t } = useTranslation();
  const stage = useRef<HTMLDivElement>(null);
  const device = useRef<HTMLDivElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const [layout, setLayout] = useState<Layout>("narrow");
  const wide = layout === "wide";
  const [hover, setHover] = useState<{ row: string | null; button: SwitchButton | null }>({ row: null, button: null });
  const [editing, setEditing] = useState<Editing | null>(null);
  const [leaders, setLeaders] = useState<Leader[]>([]);
  const [size, setSize] = useState({ width: 0, height: 0 });

  const binding = (button: SwitchButton): Binding =>
    bindings.find((item) => item.button === button) ?? { button, codes: [] };

  const activeRow = editing ? ROW_OF_BUTTON[editing.button] : hover.row;
  const activeButton = editing ? editing.button : hover.button;

  const measure = useCallback(() => {
    const host = stage.current;
    const art = device.current;
    if (!host || !art) {
      return;
    }
    const box = host.getBoundingClientRect();
    setLayout(layoutFor(box.width));
    setSize({ width: box.width, height: box.height });
    if (box.width < WIDE_MIN) {
      setLeaders([]);
      return;
    }
    const artBox = art.getBoundingClientRect();
    const scale = artBox.width / VIEW_W;
    const next: Leader[] = [];
    for (const row of MAP_ROWS) {
      const node = host.querySelector<HTMLElement>(`[data-row="${row.id}"]`);
      if (!node) {
        continue;
      }
      const rect = node.getBoundingClientRect();
      const ax = artBox.left - box.left + (row.anchor[0] - VIEW_X) * scale;
      const ay = artBox.top - box.top + (row.anchor[1] - VIEW_Y) * scale;
      const ry = rect.top - box.top + rect.height / 2;
      const left = row.side === "left";
      const rx = left ? rect.right - box.left : rect.left - box.left;
      const edge = left ? artBox.left - box.left - 10 : artBox.right - box.left + 10;
      next.push({ row: row.id, d: `M${rx} ${ry} L${edge} ${ay} L${ax} ${ay}`, ax, ay, rx, ry });
    }
    setLeaders(next);
  }, []);

  useLayoutEffect(() => {
    measure();
    const observer = new ResizeObserver(measure);
    if (stage.current) {
      observer.observe(stage.current);
    }
    return () => observer.disconnect();
  }, [measure]);

  useLayoutEffect(() => {
    measure();
  }, [measure, layout, pad, bindings]);

  useEffect(() => {
    if (!editing) {
      return;
    }
    const outside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (popover.current?.contains(target)) {
        return;
      }
      if ((target as Element).closest?.("[data-chip]")) {
        return;
      }
      setEditing(null);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [editing]);

  const openEditor = (button: SwitchButton) => {
    const host = stage.current;
    const chip = host?.querySelector<HTMLElement>(`[data-chip="${button}"]`);
    if (!host || !chip) {
      return;
    }
    if (editing?.button === button) {
      setEditing(null);
      return;
    }
    const box = host.getBoundingClientRect();
    const rect = chip.getBoundingClientRect();
    setEditing({ button, left: Math.max(0, Math.min(rect.left - box.left, box.width - 250)), top: rect.bottom - box.top + 6 });
  };

  const commit = useCallback(
    (codes: number[]) => {
      if (editing) {
        onChange(editing.button, codes);
      }
      setEditing(null);
    },
    [editing, onChange],
  );

  const renderRow = (row: MapRow) => {
    const group = row.cells.length > 1;
    const on = row.id === activeRow;
    const chips = row.cells.map((cell) => {
      const codes = binding(cell.button).codes;
      const mark = cell.at === "c" ? t("controls.press") : cell.mark;
      const lit = activeButton === cell.button;
      return (
        <button
          key={cell.button}
          type="button"
          data-chip={cell.button}
          className={[
            styles.chip,
            cell.at ? styles[cell.at] : "",
            lit ? styles.chipOn : "",
            editing?.button === cell.button ? styles.chipEditing : "",
            codes.length === 0 ? styles.chipEmpty : "",
          ]
            .filter(Boolean)
            .join(" ")}
          title={`${t(`switch.${cell.button}`)}: ${shortChordLabel(codes, t, pad)}`}
          onPointerEnter={() => setHover({ row: row.id, button: cell.button })}
          onClick={() => openEditor(cell.button)}
        >
          {mark ? <span className={styles.chipMark}>{mark}</span> : null}
          <span className={styles.chipLabel}>{shortChordLabel(codes, t, pad)}</span>
        </button>
      );
    });
    return (
      <div
        key={row.id}
        data-row={row.id}
        className={[styles.row, group ? styles.group : "", on ? styles.rowOn : ""].filter(Boolean).join(" ")}
        onPointerEnter={() => setHover({ row: row.id, button: null })}
      >
        <div className={styles.rowHead}>
          <Glyph name={row.glyph} />
          <div className={styles.rowName}>
            {t(`controls.rows.${row.id}.name`)}
            <small>{t(`controls.rows.${row.id}.hint`)}</small>
          </div>
        </div>
        <div className={styles.rowBody}>{group ? <div className={styles.cross}>{chips}</div> : chips}</div>
      </div>
    );
  };

  const editingBinding = editing ? binding(editing.button) : null;

  return (
    <div
      ref={stage}
      className={`${styles.stage} ${styles[layout]}`}
      onPointerLeave={() => setHover({ row: null, button: null })}
    >
      {wide ? (
        <svg className={styles.leaders} viewBox={`0 0 ${size.width} ${size.height}`} aria-hidden="true">
          {leaders.map((leader) => (
            <g key={leader.row} className={leader.row === activeRow ? styles.leaderOn : undefined}>
              <path d={leader.d} />
              <circle cx={leader.ax} cy={leader.ay} r={2.4} />
              <circle cx={leader.rx} cy={leader.ry} r={2} />
            </g>
          ))}
        </svg>
      ) : null}
      <div className={`${styles.rows} ${styles.left}`}>{MAP_ROWS.filter((row) => row.side === "left").map(renderRow)}</div>
      <div ref={device} className={styles.art}>
        <SwitchDevice
          activeRow={activeRow}
          activeButton={activeButton}
          onHover={(row, button) => setHover({ row, button })}
          onPick={openEditor}
        />
      </div>
      <div className={`${styles.rows} ${styles.right}`}>{MAP_ROWS.filter((row) => row.side === "right").map(renderRow)}</div>
      {editing && editingBinding ? (
        <BindingPopover
          ref={popover}
          binding={editingBinding}
          pad={pad}
          left={editing.left}
          top={editing.top}
          onChange={commit}
        />
      ) : null}
    </div>
  );
}
