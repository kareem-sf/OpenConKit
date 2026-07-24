import type { Finding } from "@openconkit/contracts";

export function supportedLocale(language: string): "en" | "ar" {
  return language.toLowerCase().startsWith("ar") ? "ar" : "en";
}

export function formatDateTime(value: string, language: string): string {
  return new Intl.DateTimeFormat(supportedLocale(language), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

export function formatNumber(value: number, language: string): string {
  return new Intl.NumberFormat(supportedLocale(language)).format(value);
}

export function formatPercent(value: number, language: string): string {
  return new Intl.NumberFormat(supportedLocale(language), {
    style: "percent",
    maximumFractionDigits: 0,
  }).format(value);
}

export function formatBytes(value: number, language: string): string {
  const units = ["B", "KB", "MB", "GB"] as const;
  let unitIndex = 0;
  let amount = value;
  while (amount >= 1024 && unitIndex < units.length - 1) {
    amount /= 1024;
    unitIndex += 1;
  }
  return `${new Intl.NumberFormat(supportedLocale(language), {
    maximumFractionDigits: unitIndex === 0 ? 0 : 1,
  }).format(amount)} ${units[unitIndex]}`;
}

export function findingLocation(finding: Finding): string {
  const suffix = finding.cell ?? finding.range?.start ?? null;
  if (finding.sheet && suffix) {
    return `${finding.sheet}!${suffix}`;
  }
  return finding.sheet ?? suffix ?? "—";
}
