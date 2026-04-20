export function formatStamp(value: string) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return value;
  }

  return new Date(numeric).toLocaleString("it-IT");
}
