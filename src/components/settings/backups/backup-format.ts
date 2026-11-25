export function megabytes(bytes: number): string {
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}

export function databaseRecords(counts?: Record<string, number>): number {
  return Object.entries(counts ?? {})
    .filter(([table]) => table !== "files" && table !== "run_logs")
    .reduce((total, [, value]) => total + value, 0);
}

export function countsTitle(counts?: Record<string, number>): string {
  return Object.entries(counts ?? {})
    .filter(([table]) => table !== "files" && table !== "run_logs")
    .map(([table, value]) => `${table}: ${value}`)
    .join("\n");
}

export function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const value = String(reader.result ?? "");
      const comma = value.indexOf(",");
      resolve(comma >= 0 ? value.slice(comma + 1) : value);
    };
    reader.onerror = () =>
      reject(reader.error ?? new Error("falha ao ler o arquivo"));
    reader.readAsDataURL(file);
  });
}
