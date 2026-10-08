import type { Crop } from "@/shared/api/bindings/Crop";
import type { IconSource } from "@/shared/api/bindings/IconSource";
import type { Metadata } from "@/shared/api/bindings/Metadata";
import { api } from "@/shared/api/commands";
import { artPalette } from "@/shared/ui/artPalette";

export const ICON_SIZE = 256;

const MAX_BYTES = 0x20000;
const QUALITIES = [0.92, 0.85, 0.75, 0.6, 0.45];
const ICON_BOX = 160;
const ICON_CENTER_Y = 104;
const FONT = '"Inter Variable", "Segoe UI", system-ui, sans-serif';
const TITLE_FONT = `600 21px ${FONT}`;
const TITLE_ONLY_FONT = `700 30px ${FONT}`;
const SIDE_PADDING = 16;

export type RenderedIcon = {
  bytes: number[];
  preview: string;
  missingExeIcon: boolean;
};

export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("image did not load"));
    image.src = src;
  });
}

export async function sourceImage(icon: IconSource, folder: string | null, executable: string | null): Promise<string | null> {
  switch (icon.kind) {
    case "executable":
      return folder && executable ? api.executableIcon(folder, executable) : null;
    case "file":
      return api.readImage(icon.path);
    case "steamGridDb":
      return api.artDownload(icon.url);
  }
}

function surface(): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const canvas = document.createElement("canvas");
  canvas.width = ICON_SIZE;
  canvas.height = ICON_SIZE;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("canvas 2d is unavailable");
  }
  return [canvas, context];
}

function effectiveCrop(image: HTMLImageElement, crop: Crop): Crop {
  const side = Math.min(image.naturalWidth, image.naturalHeight);
  if (crop.size <= 0) {
    return {
      x: Math.round((image.naturalWidth - side) / 2),
      y: Math.round((image.naturalHeight - side) / 2),
      size: side,
    };
  }
  return crop;
}

export function drawCropped(image: HTMLImageElement, crop: Crop): HTMLCanvasElement {
  const [canvas, context] = surface();
  const area = effectiveCrop(image, crop);
  context.fillStyle = "#000";
  context.fillRect(0, 0, ICON_SIZE, ICON_SIZE);
  context.imageSmoothingQuality = "high";
  context.drawImage(image, area.x, area.y, area.size, area.size, 0, 0, ICON_SIZE, ICON_SIZE);
  return canvas;
}

function averageHue(image: HTMLImageElement): { hue: number; saturation: number } | null {
  const sample = document.createElement("canvas");
  sample.width = 16;
  sample.height = 16;
  const context = sample.getContext("2d", { willReadFrequently: true });
  if (!context) {
    return null;
  }
  context.drawImage(image, 0, 0, 16, 16);
  const pixels = context.getImageData(0, 0, 16, 16).data;
  let red = 0;
  let green = 0;
  let blue = 0;
  let weight = 0;
  for (let at = 0; at < pixels.length; at += 4) {
    const alpha = (pixels[at + 3] ?? 0) / 255;
    red += (pixels[at] ?? 0) * alpha;
    green += (pixels[at + 1] ?? 0) * alpha;
    blue += (pixels[at + 2] ?? 0) * alpha;
    weight += alpha;
  }
  if (weight < 1) {
    return null;
  }
  const [r, g, b] = [red / weight / 255, green / weight / 255, blue / weight / 255];
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  if (delta === 0) {
    return { hue: 0, saturation: 0 };
  }
  const lightness = (max + min) / 2;
  const saturation = delta / (1 - Math.abs(2 * lightness - 1));
  const sector = max === r ? ((g - b) / delta) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
  return { hue: (sector * 60 + 360) % 360, saturation };
}

function background(context: CanvasRenderingContext2D, image: HTMLImageElement | null, title: string) {
  const gradient = context.createLinearGradient(0, 0, ICON_SIZE * 0.6, ICON_SIZE);
  const tone = image ? averageHue(image) : null;
  if (tone) {
    const saturation = Math.round(Math.min(0.6, Math.max(0.25, tone.saturation)) * 100);
    gradient.addColorStop(0, `hsl(${Math.round(tone.hue)} ${saturation}% 34%)`);
    gradient.addColorStop(1, `hsl(${Math.round(tone.hue)} ${saturation}% 14%)`);
  } else {
    const [from, to] = artPalette(title);
    gradient.addColorStop(0, from);
    gradient.addColorStop(1, to);
  }
  context.fillStyle = gradient;
  context.fillRect(0, 0, ICON_SIZE, ICON_SIZE);
}

function wrap(context: CanvasRenderingContext2D, text: string, maxWidth: number, maxLines: number): string[] {
  const lines: string[] = [];
  let current = "";
  for (const word of text.trim().split(/\s+/)) {
    const candidate = current ? `${current} ${word}` : word;
    if (context.measureText(candidate).width <= maxWidth || !current) {
      current = candidate;
    } else {
      lines.push(current);
      current = word;
    }
  }
  if (current) {
    lines.push(current);
  }
  if (lines.length <= maxLines) {
    return lines.map((line) => ellipsize(context, line, maxWidth));
  }
  const kept = lines.slice(0, maxLines);
  kept[maxLines - 1] = ellipsize(context, `${kept[maxLines - 1] ?? ""} ${lines.slice(maxLines).join(" ")}`, maxWidth);
  return kept.map((line) => ellipsize(context, line, maxWidth));
}

function ellipsize(context: CanvasRenderingContext2D, line: string, maxWidth: number): string {
  if (context.measureText(line).width <= maxWidth) {
    return line;
  }
  let cut = line;
  while (cut.length > 1 && context.measureText(`${cut}…`).width > maxWidth) {
    cut = cut.slice(0, -1);
  }
  return `${cut.trimEnd()}…`;
}

function drawLines(context: CanvasRenderingContext2D, lines: string[], centerY: number, lineHeight: number) {
  context.fillStyle = "#fff";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.shadowColor = "rgba(0, 0, 0, 0.45)";
  context.shadowBlur = 6;
  const top = centerY - ((lines.length - 1) * lineHeight) / 2;
  lines.forEach((line, index) => context.fillText(line, ICON_SIZE / 2, top + index * lineHeight));
  context.shadowBlur = 0;
}

export async function drawExecutable(image: HTMLImageElement | null, title: string): Promise<HTMLCanvasElement> {
  await Promise.all([document.fonts.load(TITLE_FONT), document.fonts.load(TITLE_ONLY_FONT)]).catch(() => undefined);
  const [canvas, context] = surface();
  background(context, image, title);
  const maxWidth = ICON_SIZE - SIDE_PADDING * 2;
  if (!image) {
    context.font = TITLE_ONLY_FONT;
    drawLines(context, wrap(context, title || "?", maxWidth, 4), ICON_SIZE / 2, 36);
    return canvas;
  }
  const side = Math.max(image.naturalWidth, image.naturalHeight);
  const factor = side <= ICON_BOX ? Math.floor(ICON_BOX / side) : ICON_BOX / side;
  const width = image.naturalWidth * factor;
  const height = image.naturalHeight * factor;
  context.imageSmoothingEnabled = factor < 1;
  context.imageSmoothingQuality = "high";
  context.drawImage(image, Math.round((ICON_SIZE - width) / 2), Math.round(ICON_CENTER_Y - height / 2), width, height);
  context.imageSmoothingEnabled = true;
  context.font = TITLE_FONT;
  const lines = wrap(context, title, maxWidth, 2);
  drawLines(context, lines, lines.length > 1 ? 218 : 224, 25);
  return canvas;
}

export async function encodeJpeg(canvas: HTMLCanvasElement): Promise<{ bytes: number[]; preview: string }> {
  for (const quality of QUALITIES) {
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", quality));
    if (blob && blob.size <= MAX_BYTES) {
      const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      return { bytes, preview: canvas.toDataURL("image/jpeg", quality) };
    }
  }
  throw new Error("icon does not fit into 128 KiB");
}

export async function renderIcon(metadata: Metadata, folder: string | null, executable: string | null): Promise<RenderedIcon> {
  const source = await sourceImage(metadata.icon, folder, executable);
  const image = source ? await loadImage(source) : null;
  const icon = metadata.icon;
  if (icon.kind === "executable") {
    const encoded = await encodeJpeg(await drawExecutable(image, metadata.title));
    return { ...encoded, missingExeIcon: image === null };
  }
  if (!image) {
    throw new Error("icon source is empty");
  }
  const encoded = await encodeJpeg(drawCropped(image, icon.crop));
  return { ...encoded, missingExeIcon: false };
}
