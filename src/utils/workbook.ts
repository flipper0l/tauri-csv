import * as XLSX from "xlsx";
import type { SheetUpload, WorkbookSelection } from "../types/workbook";

export function workbookNameFromPath(path: string) {
  return path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") || "Workbook";
}

function normalizeSheetRows(rawRows: unknown[][]): { columns: string[]; rows: string[][] } {
  const width = rawRows.reduce((max, row) => Math.max(max, row.length), 0);
  if (width === 0) {
    return { columns: [], rows: [] };
  }

  const headerRow = (rawRows[0] ?? []).map((cell) => String(cell ?? "").trim());
  const columns = Array.from({ length: width }, (_, index) => {
    const header = headerRow[index] ?? "";
    return header || `Column ${index + 1}`;
  });

  const rows = rawRows
    .slice(1)
    .map((row) => Array.from({ length: width }, (_, index) => String(row[index] ?? "").trim()))
    .filter((row) => row.some((value) => value.length > 0));

  return { columns, rows };
}

export function workbookToSelection(workbook: XLSX.WorkBook, sourceName: string, sourcePath: string) {
  const sheets = workbook.SheetNames.map((sheetName) => {
    const sheet = workbook.Sheets[sheetName];
    const rawRows = XLSX.utils.sheet_to_json(sheet, {
      header: 1,
      raw: false,
      defval: "",
      blankrows: false,
    }) as unknown[][];
    const normalized = normalizeSheetRows(rawRows);

    return {
      name: sheetName,
      columns: normalized.columns,
      rows: normalized.rows,
    } satisfies SheetUpload;
  }).filter((sheet) => sheet.columns.length > 0);

  return {
    path: sourcePath,
    name: sourceName,
    sheets,
  } satisfies WorkbookSelection;
}
