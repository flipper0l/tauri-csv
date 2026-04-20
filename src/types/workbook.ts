export type SheetSummary = {
  name: string;
  columns: number;
  rows: number;
};

export type VersionMeta = {
  id: string;
  label: string;
  createdAt: string;
  source: string;
  changeCount: number;
};

export type ProjectView = {
  projectId: string;
  name: string;
  importedAt: string;
  originalPath: string;
  sheets: SheetSummary[];
  versions: VersionMeta[];
  activeVersionId: string;
};

export type ColumnFilter = {
  column: string;
  value: string;
};

export type PreviewRow = {
  key: string;
  cells: string[];
};

export type SheetPreview = {
  sheetName: string;
  columns: string[];
  rows: PreviewRow[];
  total: number;
  page: number;
  pageSize: number;
};

export type ReviewField = {
  column: string;
  value: string;
};

export type PendingReviewItem = {
  sheetName: string;
  rowKey: string;
  rowIndex: number;
  note: string;
  fields: ReviewField[];
};

export type NotesImportResult = {
  baseVersionId: string;
  totalRowsWithNotes: number;
  matchedRows: number;
  unmatchedRows: number;
  warnings: string[];
  reviewItems: PendingReviewItem[];
};

export type ReviewedWizardItem = PendingReviewItem & {
  apply: boolean;
};

export type SheetUpload = {
  name: string;
  columns: string[];
  rows: string[][];
};

export type WorkbookSelection = {
  path: string;
  name: string;
  sheets: SheetUpload[];
};

export type ExportSheetData = {
  name: string;
  columns: string[];
  rows: string[][];
};

export type ExportResult = {
  workbookName: string;
  versionLabel: string;
  sheets: ExportSheetData[];
};
