import { type PointerEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Crop } from "@/shared/api/bindings/Crop";
import styles from "./IconCropper.module.css";

const VIEW = 224;
const MAX_ZOOM = 4;

type IconCropperProps = {
  src: string;
  initial?: Crop | null;
  onChange: (crop: Crop) => void;
};

type Frame = { width: number; height: number; zoom: number; x: number; y: number };

function clamp(frame: Frame): Frame {
  const scale = (VIEW / Math.min(frame.width, frame.height)) * frame.zoom;
  const minX = VIEW - frame.width * scale;
  const minY = VIEW - frame.height * scale;
  return { ...frame, x: Math.min(0, Math.max(minX, frame.x)), y: Math.min(0, Math.max(minY, frame.y)) };
}

function fromCrop(width: number, height: number, crop: Crop | null | undefined): Frame {
  const base = VIEW / Math.min(width, height);
  if (!crop || crop.size <= 0) {
    return { width, height, zoom: 1, x: (VIEW - width * base) / 2, y: (VIEW - height * base) / 2 };
  }
  const scale = VIEW / crop.size;
  const zoom = Math.min(MAX_ZOOM, Math.max(1, scale / base));
  return clamp({ width, height, zoom, x: -crop.x * scale, y: -crop.y * scale });
}

function toCrop(frame: Frame): Crop {
  const scale = (VIEW / Math.min(frame.width, frame.height)) * frame.zoom;
  return { x: Math.round(-frame.x / scale), y: Math.round(-frame.y / scale), size: Math.round(VIEW / scale) };
}

export function IconCropper({ src, initial, onChange }: IconCropperProps) {
  const { t } = useTranslation();
  const [frame, setFrame] = useState<Frame | null>(null);
  const drag = useRef<{ startX: number; startY: number; frame: Frame } | null>(null);
  const image = useRef<HTMLImageElement>(null);

  useEffect(() => {
    const element = image.current;
    if (!element) {
      return;
    }
    let cancelled = false;
    if (!element.complete) {
      setFrame(null);
    }
    element
      .decode()
      .then(() => {
        if (!cancelled && element.naturalWidth > 0) {
          setFrame(fromCrop(element.naturalWidth, element.naturalHeight, initial));
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [src]);

  useEffect(() => {
    if (frame) {
      onChange(toCrop(frame));
    }
  }, [frame, onChange]);

  const scale = frame ? (VIEW / Math.min(frame.width, frame.height)) * frame.zoom : 1;

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (!frame) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { startX: event.clientX, startY: event.clientY, frame };
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start) {
      return;
    }
    setFrame(clamp({ ...start.frame, x: start.frame.x + event.clientX - start.startX, y: start.frame.y + event.clientY - start.startY }));
  };

  const setZoom = (zoom: number) => {
    if (!frame) {
      return;
    }
    const center = VIEW / 2;
    const ratio = zoom / frame.zoom;
    setFrame(clamp({ ...frame, zoom, x: center - (center - frame.x) * ratio, y: center - (center - frame.y) * ratio }));
  };

  return (
    <div className={styles.cropper}>
      <div
        className={styles.view}
        style={{ width: VIEW, height: VIEW }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={() => {
          drag.current = null;
        }}
        role="img"
        aria-label={t("look.cropArea")}
      >
        <img
          ref={image}
          src={src}
          alt=""
          draggable={false}
          style={
            frame
              ? { width: frame.width * scale, height: frame.height * scale, transform: `translate(${frame.x}px, ${frame.y}px)` }
              : { opacity: 0 }
          }
        />
      </div>
      <label className={styles.zoom}>
        {t("look.zoom")}
        <input
          type="range"
          min={1}
          max={MAX_ZOOM}
          step={0.01}
          value={frame?.zoom ?? 1}
          disabled={!frame}
          onChange={(event) => setZoom(Number(event.target.value))}
        />
      </label>
      <p className={styles.hint}>{t("look.cropHint")}</p>
    </div>
  );
}
