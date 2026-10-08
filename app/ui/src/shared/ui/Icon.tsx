const paths = {
  gear: "M9.7 5.07L10.11 2.69L13.89 2.69L14.3 5.07L15.28 5.48L17.25 4.08L19.92 6.75L18.52 8.72L18.93 9.7L21.31 10.11L21.31 13.89L18.93 14.3L18.52 15.28L19.92 17.25L17.25 19.92L15.28 18.52L14.3 18.93L13.89 21.31L10.11 21.31L9.7 18.93L8.72 18.52L6.75 19.92L4.08 17.25L5.48 15.28L5.07 14.3L2.69 13.89L2.69 10.11L5.07 9.7L5.48 8.72L4.08 6.75L6.75 4.08L8.72 5.48ZM15 12a3 3 0 1 1-6 0a3 3 0 1 1 6 0Z",
  back: "M15 5l-7 7 7 7",
  plus: "M12 5v14M5 12h14",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  install: "M12 4v11M7 10l5 5 5-5M5 20h14",
  rebuild: "M20 11a8 8 0 0 0-14.6-4.5M4 4v4h4M4 13a8 8 0 0 0 14.6 4.5M20 20v-4h-4",
  logs: "M7 3h7l5 5v13H7zM14 3v5h5M10 13h6M10 17h6",
  trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
  check: "M5 12.5l4.5 4.5L19 7.5",
  alert: "M12 8v5M12 16.5v.5M10.3 3.9L2.6 17.5A2 2 0 0 0 4.3 20.5h15.4a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
  info: "M12 11v6M12 7.5v.5M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z",
  key: "M15 9a4 4 0 1 0-3.9 4.9L11 14l-2 2H7v2H5v2H2v-3l7.1-7.1A4 4 0 0 0 15 9zM16.5 7.5h.01",
  switch: "M7 3h4v18H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4zM13 3h4a4 4 0 0 1 4 4v10a4 4 0 0 1-4 4h-4zM7 8h.01M17 14h.01",
  sun: "M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM12 2.5v2M12 19.5v2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M2.5 12h2M19.5 12h2M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4",
  moon: "M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z",
} as const;

export type IconName = keyof typeof paths;

type IconProps = {
  name: IconName;
  size?: number;
};

export function Icon({ name, size = 18 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={paths[name]} />
    </svg>
  );
}
