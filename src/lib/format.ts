export function formatDuration(milliseconds: number, locale?: string): string {
  const totalMinutes = Math.max(0, Math.floor(milliseconds / 60_000));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  const resolvedLocale = locale ?? (
    typeof document !== "undefined" ? document.documentElement.lang : undefined
  );
  const chinese = resolvedLocale === "zh-CN";

  if (chinese) {
    if (hours === 0) return `${minutes}分钟`;
    if (minutes === 0) return `${hours}小时`;
    return `${hours}小时 ${minutes}分钟`;
  }
  if (hours === 0) return `${minutes}m`;
  if (minutes === 0) return `${hours}h`;
  return `${hours}h ${minutes}m`;
}

export function formatClock(timestamp: number | null, locale?: string): string {
  if (timestamp === null) return "—";
  const resolvedLocale = locale ?? (
    typeof document !== "undefined" ? document.documentElement.lang : undefined
  );
  return new Intl.DateTimeFormat(resolvedLocale, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(timestamp));
}

export function formatLongDate(date: Date, locale?: string): string {
  return new Intl.DateTimeFormat(locale, {
    weekday: "long",
    month: "long",
    day: "numeric",
  }).format(date);
}

export function localIsoDate(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function dateFromLocalIso(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day, 12);
}

export function shiftLocalDate(value: string, days: number): string {
  const date = dateFromLocalIso(value);
  date.setDate(date.getDate() + days);
  return localIsoDate(date);
}
