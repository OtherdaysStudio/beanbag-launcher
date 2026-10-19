const hours = new Intl.NumberFormat(undefined, {
  style: "unit",
  unit: "hour",
  unitDisplay: "long",
  maximumFractionDigits: 0,
});
const minutes = new Intl.NumberFormat(undefined, {
  style: "unit",
  unit: "minute",
  unitDisplay: "long",
  maximumFractionDigits: 0,
});
const gigabytes = new Intl.NumberFormat(undefined, {
  style: "unit",
  unit: "gigabyte",
  unitDisplay: "short",
  maximumFractionDigits: 1,
});
const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const clock = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });

export function playtime(mins: number): string {
  if (mins <= 0) return "";
  return mins < 60 ? minutes.format(mins) : hours.format(Math.round(mins / 60));
}

export function size(bytes: number): string {
  if (bytes <= 0) return "";
  return gigabytes.format(bytes / 1e9);
}

/** "yesterday", "3 days ago", "last month". Empty when never played. */
export function lastPlayed(unixSeconds: number, now = Date.now()): string {
  if (unixSeconds <= 0) return "";
  const days = Math.round((unixSeconds * 1000 - now) / 86_400_000);
  if (days > -1) return relative.format(0, "day");
  if (days > -7) return relative.format(days, "day");
  if (days > -30) return relative.format(Math.round(days / 7), "week");
  if (days > -365) return relative.format(Math.round(days / 30), "month");
  return relative.format(Math.round(days / 365), "year");
}

export function time(date: Date): string {
  return clock.format(date);
}
