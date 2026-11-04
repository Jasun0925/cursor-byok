const integerFormatter = new Intl.NumberFormat("en-US");

const compactUnits = [
  { value: 1_000_000_000_000, suffix: "T" },
  { value: 1_000_000_000, suffix: "B" },
  { value: 1_000_000, suffix: "M" },
  { value: 1_000, suffix: "K" },
] as const;

function normalizeInteger(value: number) {
  return Number.isFinite(value) ? Math.round(value) : 0;
}

function trimTrailingZeros(value: string) {
  return value.replace(/\.0$/, "").replace(/(\.\d*[1-9])0+$/, "$1");
}

export function formatInteger(value: number) {
  return integerFormatter.format(normalizeInteger(value));
}

export function formatCompactInteger(value: number) {
  const number = normalizeInteger(value);
  const unit = compactUnits.find(({ value: threshold }) => Math.abs(number) >= threshold);
  if (!unit) return formatInteger(number);

  const scaled = number / unit.value;
  const fractionDigits = Math.abs(scaled) < 100 ? 1 : 0;
  return `${trimTrailingZeros(scaled.toFixed(fractionDigits))}${unit.suffix}`;
}

export function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : value.toFixed(0)} ${units[unit]}`;
}
