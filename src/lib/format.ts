import { i18n } from "./i18n.svelte";

const locale = () => (i18n.lang === "fr" ? "fr-FR" : "en-US");

export function formatSize(bytes: number): string {
  const units = i18n.lang === "fr" ? ["o", "Ko", "Mo", "Go", "To"] : ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  const digits = value >= 100 || i === 0 ? 0 : 1;
  return `${value.toLocaleString(locale(), { maximumFractionDigits: digits })} ${units[i]}`;
}

export function relativeTime(ms: number | null): string {
  if (ms == null) return "—";
  const rtf = new Intl.RelativeTimeFormat(locale(), { numeric: "auto" });
  const diff = (ms - Date.now()) / 1000;
  const steps: [Intl.RelativeTimeFormatUnit, number][] = [
    ["second", 60],
    ["minute", 60],
    ["hour", 24],
    ["day", 7],
    ["week", 4.35],
    ["month", 12],
    ["year", Infinity],
  ];
  let value = diff;
  for (const [unit, size] of steps) {
    if (Math.abs(value) < size) return rtf.format(Math.round(value), unit);
    value /= size;
  }
  return rtf.format(Math.round(value), "year");
}

export function formatDate(ms: number): string {
  return new Date(ms).toLocaleString(locale(), {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Couleur de jaquette générée, stable pour un titre donné. */
export function titleColor(title: string): string {
  let h = 0;
  for (const c of title) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return `hsl(${h % 360} 32% 22%)`;
}
