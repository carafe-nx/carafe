import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { SwitchButton } from "@/shared/api/bindings/SwitchButton";
import { DEVICE_VIEWBOX, ROW_OF_BUTTON } from "./controllerLayout";
import styles from "./ControllerMap.module.css";

type SwitchDeviceProps = {
  activeRow: string | null;
  activeButton: SwitchButton | null;
  onHover: (row: string | null, button: SwitchButton | null) => void;
  onPick: (button: SwitchButton) => void;
};

type Handlers = Pick<SwitchDeviceProps, "onHover" | "onPick">;

type Lit = (button: SwitchButton) => boolean;

function arrowPoints(x: number, y: number, dx: number, dy: number, size: number): string {
  const px = -dy;
  const py = dx;
  const tip = `${x + dx * size},${y + dy * size}`;
  const left = `${x - dx * size * 0.65 + px * size},${y - dy * size * 0.65 + py * size}`;
  const right = `${x - dx * size * 0.65 - px * size},${y - dy * size * 0.65 - py * size}`;
  return `${tip} ${left} ${right}`;
}

const DIRECTIONS = { n: [0, -1], s: [0, 1], w: [-1, 0], e: [1, 0] } as const;

function classes(on: boolean, extra?: string): string {
  return [styles.part, on ? styles.on : "", extra ?? ""].filter(Boolean).join(" ");
}

function Single({
  button,
  lit,
  handlers,
  children,
}: {
  button: SwitchButton;
  lit: Lit;
  handlers: Handlers;
  children: ReactNode;
}) {
  return (
    <g
      className={classes(lit(button))}
      onPointerEnter={() => handlers.onHover(ROW_OF_BUTTON[button], button)}
      onClick={() => handlers.onPick(button)}
    >
      {children}
    </g>
  );
}

function Stick({
  cx,
  cy,
  ids,
  lit,
  handlers,
}: {
  cx: number;
  cy: number;
  ids: Record<"n" | "s" | "w" | "e" | "c", SwitchButton>;
  lit: Lit;
  handlers: Handlers;
}) {
  const row = ROW_OF_BUTTON[ids.c];
  return (
    <g onPointerEnter={() => handlers.onHover(row, null)}>
      <circle className={styles.hit} cx={cx} cy={cy} r={15} />
      <g
        className={classes(lit(ids.c))}
        onPointerEnter={() => handlers.onHover(row, ids.c)}
        onClick={() => handlers.onPick(ids.c)}
      >
        <circle className={styles.ring} cx={cx} cy={cy} r={10} />
        <circle className={styles.key} cx={cx} cy={cy} r={6.6} />
      </g>
      {(Object.keys(DIRECTIONS) as (keyof typeof DIRECTIONS)[]).map((dir) => {
        const [dx, dy] = DIRECTIONS[dir];
        const button = ids[dir];
        return (
          <g
            key={dir}
            className={classes(lit(button))}
            onPointerEnter={() => handlers.onHover(row, button)}
            onClick={() => handlers.onPick(button)}
          >
            <circle className={styles.hit} cx={cx + dx * 13} cy={cy + dy * 13} r={3} />
            <polygon className={styles.key} points={arrowPoints(cx + dx * 13, cy + dy * 13, dx, dy, 1.5)} />
          </g>
        );
      })}
    </g>
  );
}

function FourButtons({
  cx,
  cy,
  ids,
  lettered,
  lit,
  handlers,
}: {
  cx: number;
  cy: number;
  ids: Record<"n" | "s" | "w" | "e", SwitchButton>;
  lettered: boolean;
  lit: Lit;
  handlers: Handlers;
}) {
  const row = ROW_OF_BUTTON[ids.n];
  return (
    <g onPointerEnter={() => handlers.onHover(row, null)}>
      <circle className={styles.hit} cx={cx} cy={cy} r={16} />
      {(Object.keys(DIRECTIONS) as (keyof typeof DIRECTIONS)[]).map((dir) => {
        const [dx, dy] = DIRECTIONS[dir];
        const button = ids[dir];
        const x = cx + dx * 9.2;
        const y = cy + dy * 9.2;
        return (
          <g
            key={dir}
            className={classes(lit(button))}
            onPointerEnter={() => handlers.onHover(row, button)}
            onClick={() => handlers.onPick(button)}
          >
            <circle className={styles.key} cx={x} cy={y} r={4.2} />
            {lettered ? (
              <text className={styles.keyText} x={x} y={y + 1.3}>
                {button}
              </text>
            ) : (
              <polygon className={styles.keyText} points={arrowPoints(x, y, dx, dy, 1.4)} />
            )}
          </g>
        );
      })}
    </g>
  );
}

export function SwitchDevice({ activeRow, activeButton, onHover, onPick }: SwitchDeviceProps) {
  const { t } = useTranslation();
  const handlers = { onHover, onPick };
  const lit: Lit = (button) =>
    ROW_OF_BUTTON[button] === activeRow && (activeButton === null || activeButton === button);

  return (
    <svg
      className={styles.device}
      viewBox={DEVICE_VIEWBOX}
      role="img"
      aria-label={t("controls.deviceLabel")}
      onPointerLeave={() => onHover(null, null)}
    >
      <Single button="ZL" lit={lit} handlers={handlers}>
        <rect className={styles.key} x={9} y={-11.5} width={24.5} height={7.5} rx={3.75} />
        <text className={styles.keyText} x={21.25} y={-6.5}>
          ZL
        </text>
      </Single>
      <Single button="ZR" lit={lit} handlers={handlers}>
        <rect className={styles.key} x={205.5} y={-11.5} width={24.5} height={7.5} rx={3.75} />
        <text className={styles.keyText} x={217.75} y={-6.5}>
          ZR
        </text>
      </Single>
      <Single button="L" lit={lit} handlers={handlers}>
        <rect className={styles.hit} x={2} y={-3.5} width={33} height={7} />
        <path className={styles.key} d="M4.6 5.8 A16.5 16.5 0 0 1 16.4 -2 H33.6 V-0.6 H16.6 A15 15 0 0 0 6 4.6 Z" />
      </Single>
      <Single button="R" lit={lit} handlers={handlers}>
        <rect className={styles.hit} x={204} y={-3.5} width={33} height={7} />
        <path className={styles.key} d="M234.4 5.8 A16.5 16.5 0 0 0 222.6 -2 H205.4 V-0.6 H222.4 A15 15 0 0 1 233 4.6 Z" />
      </Single>

      <path className={styles.shell} d="M35.9 0 H16 A16 16 0 0 0 0 16 V86 A16 16 0 0 0 16 102 H35.9 Z" />
      <path className={styles.shell} d="M203.1 0 H223 A16 16 0 0 1 239 16 V86 A16 16 0 0 1 223 102 H203.1 Z" />
      <rect className={styles.shell} x={35.9} y={0} width={167.2} height={102} />
      <rect className={styles.screen} x={43.5} y={8} width={152} height={86} rx={1.5} />

      <Single button="MINUS" lit={lit} handlers={handlers}>
        <rect className={styles.hit} x={23} y={4} width={12} height={9} />
        <rect className={styles.key} x={25.8} y={7.6} width={6.4} height={1.9} rx={0.95} />
      </Single>
      <Single button="PLUS" lit={lit} handlers={handlers}>
        <rect className={styles.hit} x={204} y={2.5} width={12} height={12} />
        <path
          className={styles.key}
          d="M209.05 6.3 a0.95 0.95 0 0 1 1.9 0 V7.6 H212.25 a0.95 0.95 0 0 1 0 1.9 H210.95 V10.8 a0.95 0.95 0 0 1 -1.9 0 V9.5 H207.75 a0.95 0.95 0 0 1 0 -1.9 H209.05 Z"
        />
      </Single>

      <Stick cx={17.5} cy={28} ids={{ n: "LUP", s: "LDOWN", w: "LLEFT", e: "LRIGHT", c: "STICKL" }} lit={lit} handlers={handlers} />
      <Stick cx={221.5} cy={60} ids={{ n: "RUP", s: "RDOWN", w: "RLEFT", e: "RRIGHT", c: "STICKR" }} lit={lit} handlers={handlers} />
      <FourButtons cx={17.5} cy={60} ids={{ n: "UP", s: "DOWN", w: "LEFT", e: "RIGHT" }} lettered={false} lit={lit} handlers={handlers} />
      <FourButtons cx={221.5} cy={28} ids={{ n: "X", s: "B", w: "Y", e: "A" }} lettered lit={lit} handlers={handlers} />

      <g aria-hidden="true">
        <rect className={styles.fixed} x={22.6} y={78.4} width={5.6} height={5.6} rx={1.2} />
        <circle className={styles.fixed} cx={213.6} cy={81.2} r={3.3} />
      </g>
    </svg>
  );
}
