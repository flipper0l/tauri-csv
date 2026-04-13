<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
import * as XLSX from "xlsx";

type SheetSummary = {
  name: string;
  columns: number;
  rows: number;
};

type VersionMeta = {
  id: string;
  label: string;
  created_at: string;
  source: string;
  change_count: number;
};

type ProjectView = {
  project_id: string;
  name: string;
  imported_at: string;
  original_path: string;
  sheets: SheetSummary[];
  versions: VersionMeta[];
  active_version_id: string;
};

type ColumnFilter = {
  column: string;
  value: string;
};

type PreviewRow = {
  key: string;
  cells: string[];
};

type SheetPreview = {
  sheet_name: string;
  columns: string[];
  rows: PreviewRow[];
  total: number;
  page: number;
  page_size: number;
};

type ReviewField = {
  column: string;
  value: string;
};

type PendingReviewItem = {
  sheet_name: string;
  row_key: string;
  row_index: number;
  note: string;
  fields: ReviewField[];
};

type NotesImportResult = {
  base_version_id: string;
  total_rows_with_notes: number;
  matched_rows: number;
  unmatched_rows: number;
  warnings: string[];
  review_items: PendingReviewItem[];
};

type ReviewedWizardItem = PendingReviewItem & {
  apply: boolean;
};

type SheetUpload = {
  name: string;
  columns: string[];
  rows: string[][];
};

type WorkbookSelection = {
  path: string;
  name: string;
  sheets: SheetUpload[];
};

type ExportSheetData = {
  name: string;
  columns: string[];
  rows: string[][];
};

type ExportResult = {
  workbook_name: string;
  version_label: string;
  sheets: ExportSheetData[];
};

const project = ref<ProjectView | null>(null);
const selectedSheet = ref("");
const selectedVersionId = ref("");
const loading = ref(false);
const busyLabel = ref("");
const successMessage = ref("");
const errorMessage = ref("");

const preview = ref<SheetPreview | null>(null);
const page = ref(1);
const pageSize = ref(25);
const filters = reactive<Record<string, string>>({});

const notesWizard = ref<NotesImportResult | null>(null);
const wizardItems = ref<ReviewedWizardItem[]>([]);
const wizardIndex = ref(0);
const versionLabel = ref("");
const originalWorkbookInput = ref<HTMLInputElement | null>(null);
const notesWorkbookInput = ref<HTMLInputElement | null>(null);
const pendingPickerTarget = ref<"original" | "notes" | null>(null);

const totalPages = computed(() => {
  if (!preview.value) {
    return 1;
  }
  return Math.max(1, Math.ceil(preview.value.total / preview.value.page_size));
});

const currentWizardItem = computed(() => wizardItems.value[wizardIndex.value] ?? null);
const activeVersion = computed(() =>
  project.value?.versions.find((version) => version.id === selectedVersionId.value) ?? null,
);

function workbookNameFromPath(path: string) {
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
    .map((row) => {
      const padded = Array.from({ length: width }, (_, index) => String(row[index] ?? "").trim());
      return padded;
    })
    .filter((row) => row.some((value) => value.length > 0));

  return { columns, rows };
}

function workbookToSelection(workbook: XLSX.WorkBook, sourceName: string, sourcePath: string) {
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

function resetFeedback() {
  successMessage.value = "";
  errorMessage.value = "";
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function openBrowserFilePicker(target: "original" | "notes") {
  pendingPickerTarget.value = target;
  await nextTick();

  const input = target === "original" ? originalWorkbookInput.value : notesWorkbookInput.value;
  input?.click();
}

async function loadWorkbookFromPath(path: string) {
  const bytes = await readFile(path);
  const workbook = XLSX.read(bytes, { type: "array" });
  return workbookToSelection(workbook, workbookNameFromPath(path), path);
}

async function loadWorkbookFromBrowserFile(file: File) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const workbook = XLSX.read(bytes, { type: "array" });
  return workbookToSelection(workbook, file.name.replace(/\.[^.]+$/, ""), file.name);
}

function resolveReturnedSheet(selection: WorkbookSelection) {
  if (selection.sheets.length === 1) {
    return selection.sheets[0];
  }

  const matchedSheet = selection.sheets.find((sheet) => sheet.name === selectedSheet.value);
  if (matchedSheet) {
    return matchedSheet;
  }

  throw new Error(
    `Il file di ritorno contiene ${selection.sheets.length} fogli. Deve corrispondere al foglio selezionato '${selectedSheet.value}'.`,
  );
}

async function importOriginalWorkbookSelection(selection: WorkbookSelection) {
  const imported = await invoke<ProjectView>("import_original_workbook", {
    payload: {
      path: selection.path,
      name: selection.name,
      sheets: selection.sheets,
    },
  });
  project.value = imported;
  selectedVersionId.value = imported.active_version_id;
  selectedSheet.value = imported.sheets[0]?.name ?? "";
  page.value = 1;
  syncFilters([]);
  await refreshPreview();
  successMessage.value = "Workbook originale importato e versionato correttamente.";
}

async function importNotesWorkbookSelection(selection: WorkbookSelection) {
  if (!project.value || !selectedVersionId.value || !selectedSheet.value) {
    return;
  }

  const result = await invoke<NotesImportResult>("import_notes_workbook", {
    payload: {
      projectId: project.value.project_id,
      versionId: selectedVersionId.value,
      sheetName: selectedSheet.value,
      sheet: resolveReturnedSheet(selection),
    },
  });

  notesWizard.value = result;
  wizardItems.value = result.review_items.map((item) => ({
    ...item,
    apply: true,
    fields: item.fields.map((field) => ({ ...field })),
  }));
  wizardIndex.value = 0;
  versionLabel.value = `Versione ${project.value.versions.length + 1}`;
  successMessage.value = `Trovate ${result.total_rows_with_notes} righe con note. ${result.matched_rows} pronte per la revisione.`;
}

async function selectWorkbookFile(target: "original" | "notes") {
  try {
    if (isTauriRuntime()) {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Excel", extensions: ["xlsx", "xlsm", "xls"] }],
      });

      if (typeof selected === "string") {
        return await loadWorkbookFromPath(selected);
      }
      return null;
    }
  } catch (error) {
    errorMessage.value = `Selettore file nativo non disponibile: ${String(error)}`;
  }

  await openBrowserFilePicker(target);
  return null;
}

async function handleBrowserFileChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  const target = pendingPickerTarget.value;

  if (!file || !target) {
    return;
  }

  try {
    loading.value = true;
    busyLabel.value = target === "original" ? "Import del file originale in corso" : "Analizzo il file di ritorno con le note";
    resetFeedback();

    const selection = await loadWorkbookFromBrowserFile(file);
    if (target === "original") {
      await importOriginalWorkbookSelection(selection);
    } else {
      await importNotesWorkbookSelection(selection);
    }
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
    busyLabel.value = "";
    input.value = "";
    pendingPickerTarget.value = null;
  }
}

function formatStamp(value: string) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return value;
  }

  return new Date(numeric).toLocaleString("it-IT");
}

function syncFilters(columns: string[]) {
  const nextEntries = Object.fromEntries(columns.map((column) => [column, filters[column] ?? ""]));

  Object.keys(filters).forEach((key) => {
    delete filters[key];
  });

  Object.entries(nextEntries).forEach(([key, value]) => {
    filters[key] = value;
  });
}

function currentFilterPayload(): ColumnFilter[] {
  return Object.entries(filters).map(([column, value]) => ({ column, value }));
}

async function loadCurrentProject() {
  const current = await invoke<ProjectView | null>("get_current_project");
  if (!current) {
    return;
  }

  project.value = current;
  selectedVersionId.value = current.active_version_id;
  selectedSheet.value = selectedSheet.value || current.sheets[0]?.name || "";
  await refreshPreview();
}

async function importOriginalWorkbook() {
  resetFeedback();

  const selected = await selectWorkbookFile("original");
  if (!selected) {
    return;
  }

  loading.value = true;
  busyLabel.value = "Import del file originale in corso";

  try {
    await importOriginalWorkbookSelection(selected);
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
    busyLabel.value = "";
  }
}

async function refreshPreview() {
  if (!project.value || !selectedSheet.value || !selectedVersionId.value) {
    preview.value = null;
    return;
  }

  loading.value = true;
  busyLabel.value = "Aggiorno l'anteprima del foglio";

  try {
    const data = await invoke<SheetPreview>("get_sheet_preview", {
      payload: {
        projectId: project.value.project_id,
        versionId: selectedVersionId.value,
        sheetName: selectedSheet.value,
        page: page.value,
        pageSize: pageSize.value,
        filters: currentFilterPayload(),
      },
    });
    preview.value = data;
    syncFilters(data.columns);
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
    busyLabel.value = "";
  }
}

async function changeSheet(sheetName: string) {
  selectedSheet.value = sheetName;
  page.value = 1;
  syncFilters([]);
  await refreshPreview();
}

async function changeVersion(versionId: string) {
  selectedVersionId.value = versionId;
  page.value = 1;
  await refreshPreview();
}

async function applyFilters() {
  page.value = 1;
  await refreshPreview();
}

async function goToPage(nextPage: number) {
  if (nextPage < 1 || nextPage > totalPages.value) {
    return;
  }

  page.value = nextPage;
  await refreshPreview();
}

async function importNotesWorkbook() {
  if (!project.value || !selectedVersionId.value) {
    return;
  }

  resetFeedback();

  const selected = await selectWorkbookFile("notes");
  if (!selected) {
    return;
  }

  loading.value = true;
  busyLabel.value = "Analizzo il file di ritorno con le note";

  try {
    await importNotesWorkbookSelection(selected);
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
    busyLabel.value = "";
  }
}

function closeWizard() {
  notesWizard.value = null;
  wizardItems.value = [];
  wizardIndex.value = 0;
}

async function saveReviewedVersion() {
  if (!project.value || !notesWizard.value) {
    return;
  }

  loading.value = true;
  busyLabel.value = "Creo una nuova versione con le modifiche revisionate";

  try {
    const result = await invoke<{ project: ProjectView; created_version_id: string }>("apply_notes_import", {
      payload: {
        projectId: project.value.project_id,
        baseVersionId: notesWizard.value.base_version_id,
        label: versionLabel.value,
        reviewedItems: wizardItems.value,
      },
    });

    project.value = result.project;
    selectedVersionId.value = result.created_version_id;
    closeWizard();
    await refreshPreview();
    successMessage.value = "Nuova versione salvata nello storico.";
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
    busyLabel.value = "";
  }
}

async function exportCurrentVersion() {
  if (!project.value || !selectedVersionId.value) {
    return;
  }

  resetFeedback();

  const target = await save({
    filters: [{ name: "Excel", extensions: ["xlsx"] }],
    defaultPath: `${project.value.name}-${activeVersion.value?.label ?? "export"}.xlsx`,
  });

  if (typeof target !== "string") {
    return;
  }

  loading.value = true;
  busyLabel.value = "Esporto la versione filtrata";

  try {
    const result = await invoke<ExportResult>("export_version", {
      payload: {
        projectId: project.value.project_id,
        versionId: selectedVersionId.value,
        filters: currentFilterPayload(),
      },
    });
    const workbook = XLSX.utils.book_new();

    result.sheets.forEach((sheet) => {
      const worksheet = XLSX.utils.aoa_to_sheet([sheet.columns, ...sheet.rows]);
      XLSX.utils.book_append_sheet(workbook, worksheet, sheet.name);
    });

    const bytes = XLSX.write(workbook, {
      bookType: "xlsx",
      type: "array",
    }) as ArrayBuffer;

    if (isTauriRuntime()) {
      await writeFile(target, new Uint8Array(bytes));
    } else {
      XLSX.writeFile(workbook, `${result.workbook_name}-${result.version_label}.xlsx`);
    }
    successMessage.value = "Export completato con i filtri attivi.";
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
    busyLabel.value = "";
  }
}

watch(selectedSheet, () => {
  if (preview.value && preview.value.sheet_name !== selectedSheet.value) {
    preview.value = null;
  }
});

onMounted(async () => {
  await loadCurrentProject();
});
</script>

<template>
  <main class="workspace">
    <input
      ref="originalWorkbookInput"
      class="hidden-input"
      type="file"
      accept=".xlsx,.xlsm,.xls"
      @change="handleBrowserFileChange"
    />
    <input
      ref="notesWorkbookInput"
      class="hidden-input"
      type="file"
      accept=".xlsx,.xlsm,.xls"
      @change="handleBrowserFileChange"
    />

    <aside class="sidebar">
      <div class="brand">
        <p class="eyebrow">Excel Review</p>
        <h1>Versiona, filtra e integra note.</h1>
      </div>

      <section class="project-card" v-if="project">
        <p class="label">Workbook attivo</p>
        <strong>{{ project.name }}</strong>
        <p class="meta">Importato il {{ formatStamp(project.imported_at) }}</p>
      </section>

      <section class="sheet-nav" v-if="project">
        <div class="sheet-nav-header">
          <h2>Fogli</h2>
          <span>{{ project.sheets.length }}</span>
        </div>

        <button
          v-for="sheet in project.sheets"
          :key="sheet.name"
          type="button"
          class="sheet-button"
          :class="{ active: selectedSheet === sheet.name }"
          @click="changeSheet(sheet.name)"
        >
          <span>{{ sheet.name }}</span>
          <small>{{ sheet.rows }} righe</small>
        </button>
      </section>

      <button type="button" class="import-button" @click="importOriginalWorkbook" :disabled="loading">
        Importa file originale
      </button>
    </aside>

    <section class="main-panel">
      <header class="topbar">
        <div>
          <p class="eyebrow">Centro operativo</p>
          <h2>{{ selectedSheet || "Nessun foglio selezionato" }}</h2>
        </div>

        <div class="actions" v-if="project">
          <label class="field compact">
            <span>Versione</span>
            <select :value="selectedVersionId" @change="changeVersion(($event.target as HTMLSelectElement).value)">
              <option v-for="version in project.versions" :key="version.id" :value="version.id">
                {{ version.label }}
              </option>
            </select>
          </label>

          <button type="button" @click="importNotesWorkbook" :disabled="loading">
            Importa file con note
          </button>
          <button type="button" @click="exportCurrentVersion" :disabled="loading">
            Esporta versione
          </button>
        </div>
      </header>

      <p v-if="busyLabel" class="status info">{{ busyLabel }}...</p>
      <p v-if="successMessage" class="status success">{{ successMessage }}</p>
      <p v-if="errorMessage" class="status error">{{ errorMessage }}</p>

      <section v-if="project && preview" class="preview-card">
        <div class="preview-toolbar">
          <div>
            <strong>{{ preview.total }}</strong>
            <span> righe trovate con i filtri attivi</span>
          </div>
          <div class="pager">
            <button type="button" @click="goToPage(page - 1)" :disabled="page <= 1 || loading">Prec.</button>
            <span>Pagina {{ page }} / {{ totalPages }}</span>
            <button type="button" @click="goToPage(page + 1)" :disabled="page >= totalPages || loading">Succ.</button>
          </div>
        </div>

        <div class="table-shell">
          <table>
            <thead>
              <tr>
                <th v-for="column in preview.columns" :key="column">{{ column }}</th>
              </tr>
              <tr class="filter-row">
                <th v-for="column in preview.columns" :key="`${column}-filter`">
                  <input
                    v-model="filters[column]"
                    type="text"
                    :placeholder="`Filtra ${column}`"
                    @keyup.enter="applyFilters"
                  />
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="preview.rows.length === 0">
                <td :colspan="preview.columns.length" class="empty">
                  Nessun record trovato.
                </td>
              </tr>
              <tr v-for="row in preview.rows" :key="row.key">
                <td v-for="(cell, index) in row.cells" :key="`${row.key}-${index}`">{{ cell }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="filter-actions">
          <button type="button" @click="applyFilters" :disabled="loading">Applica filtri</button>
          <button
            type="button"
            @click="
              syncFilters(preview.columns);
              applyFilters();
            "
            :disabled="loading"
          >
            Reset filtri
          </button>
        </div>
      </section>

      <section v-else class="empty-state">
        <h3>Importa un workbook per iniziare</h3>
        <p>
          L'app creerà uno storico delle versioni, mostrerà i fogli nella sidebar e ti permetterà di
          revisionare le note ricevute con un wizard dedicato.
        </p>
      </section>

      <section v-if="project" class="history-card">
        <div class="history-header">
          <h3>Storico versioni</h3>
          <span>{{ project.versions.length }} versioni</span>
        </div>

        <div class="history-list">
          <article
            v-for="version in [...project.versions].reverse()"
            :key="version.id"
            class="history-item"
            :class="{ active: version.id === selectedVersionId }"
          >
            <div>
              <strong>{{ version.label }}</strong>
              <p>{{ version.source }}</p>
            </div>
            <div class="history-meta">
              <span>{{ version.change_count }} modifiche</span>
              <small>{{ formatStamp(version.created_at) }}</small>
            </div>
          </article>
        </div>
      </section>
    </section>

    <div v-if="notesWizard" class="modal-backdrop">
      <section class="modal">
        <header class="modal-header">
          <div>
            <p class="eyebrow">Wizard note</p>
            <h3>Revisione record con commenti</h3>
          </div>
          <button type="button" class="ghost" @click="closeWizard">Chiudi</button>
        </header>

        <div class="wizard-summary">
          <p class="summary-copy">
            {{ notesWizard.matched_rows }} righe abbinate, {{ notesWizard.unmatched_rows }} non abbinate.
          </p>
          <label class="field">
            <span>Nome nuova versione</span>
            <input v-model="versionLabel" type="text" />
          </label>
          <div v-if="notesWizard.warnings.length > 0" class="warnings">
            <p v-for="warning in notesWizard.warnings" :key="warning">{{ warning }}</p>
          </div>
        </div>

        <div v-if="currentWizardItem" class="wizard-body">
          <div class="wizard-nav">
            <button type="button" @click="wizardIndex = Math.max(0, wizardIndex - 1)" :disabled="wizardIndex === 0">
              Prec.
            </button>
            <span>Record {{ wizardIndex + 1 }} / {{ wizardItems.length }}</span>
            <button
              type="button"
              @click="wizardIndex = Math.min(wizardItems.length - 1, wizardIndex + 1)"
              :disabled="wizardIndex >= wizardItems.length - 1"
            >
              Succ.
            </button>
          </div>

          <div class="note-card">
            <p class="label">Foglio</p>
            <strong>{{ currentWizardItem.sheet_name }}</strong>
            <p class="label">Nota ricevuta</p>
            <p class="note">{{ currentWizardItem.note }}</p>
            <label class="checkbox">
              <input v-model="currentWizardItem.apply" type="checkbox" />
              <span>Applica questa modifica alla nuova versione</span>
            </label>
          </div>

          <div class="form-grid">
            <label v-for="field in currentWizardItem.fields" :key="`${currentWizardItem.row_key}-${field.column}`" class="field">
              <span>{{ field.column }}</span>
              <input v-model="field.value" type="text" />
            </label>
          </div>
        </div>

        <footer class="modal-footer">
          <button type="button" class="ghost" @click="closeWizard">Annulla</button>
          <button type="button" @click="saveReviewedVersion" :disabled="loading || wizardItems.length === 0">
            Salva nuova versione
          </button>
        </footer>
      </section>
    </div>
  </main>
</template>

<style scoped>
:global(body) {
  margin: 0;
  font-family: "Segoe UI", "Avenir Next", sans-serif;
  background:
    radial-gradient(circle at top left, rgba(255, 219, 173, 0.35), transparent 32%),
    radial-gradient(circle at bottom right, rgba(129, 185, 164, 0.28), transparent 28%),
    #f4efe6;
  color: #1f2a2a;
}

:global(*) {
  box-sizing: border-box;
}

.workspace {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
}

.hidden-input {
  display: none;
}

.sidebar {
  padding: 1.5rem;
  background: rgba(28, 46, 48, 0.95);
  color: #f8f4ea;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.brand h1,
.topbar h2,
.empty-state h3,
.history-header h3,
.modal-header h3 {
  margin: 0;
}

.eyebrow,
.label {
  text-transform: uppercase;
  letter-spacing: 0.08em;
  font-size: 0.72rem;
  color: #8ba7a4;
}

.project-card,
.preview-card,
.history-card,
.empty-state,
.modal,
.note-card {
  background: #fffbf5;
  border: 1px solid rgba(93, 117, 113, 0.18);
  border-radius: 20px;
  box-shadow: 0 18px 45px rgba(30, 43, 42, 0.09);
}

.project-card {
  padding: 1rem;
}

.meta {
  margin-bottom: 0;
  color: #56706c;
}

.sheet-nav {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  min-height: 0;
}

.sheet-nav-header,
.history-header,
.preview-toolbar,
.topbar,
.modal-header,
.modal-footer,
.wizard-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.sheet-button,
button,
select,
input {
  font: inherit;
}

.sheet-button,
button {
  border: 1px solid rgba(133, 104, 63, 0.28);
  background: #fff8ed;
  color: #243030;
  border-radius: 14px;
  cursor: pointer;
  transition: transform 0.2s ease, background 0.2s ease;
}

.sheet-button:hover,
button:hover {
  transform: translateY(-1px);
}

.sheet-button {
  padding: 0.9rem 1rem;
  text-align: left;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 0.75rem;
}

.sheet-button.active {
  background: linear-gradient(135deg, #f5d4a5, #f7f2e6);
}

.import-button {
  margin-top: auto;
  padding: 1rem;
  background: linear-gradient(135deg, #f1c98f, #f4ebda);
}

.main-panel {
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  align-items: end;
}

.field {
  display: grid;
  gap: 0.35rem;
}

.field.compact {
  min-width: 220px;
}

select,
input,
button {
  padding: 0.72rem 0.9rem;
}

select,
input {
  border: 1px solid rgba(91, 111, 105, 0.24);
  border-radius: 12px;
  background: #fff;
}

.status {
  margin: 0;
  padding: 0.85rem 1rem;
  border-radius: 14px;
}

.status.info {
  background: rgba(240, 218, 183, 0.5);
}

.status.success {
  background: rgba(188, 221, 205, 0.6);
}

.status.error {
  background: rgba(236, 184, 171, 0.7);
}

.preview-card,
.history-card,
.empty-state {
  padding: 1.2rem;
}

.table-shell {
  overflow: auto;
  margin-top: 1rem;
  border-radius: 16px;
  border: 1px solid rgba(88, 104, 100, 0.14);
}

table {
  width: 100%;
  border-collapse: collapse;
  min-width: 920px;
  background: rgba(255, 255, 255, 0.9);
}

th,
td {
  padding: 0.8rem;
  text-align: left;
  border-bottom: 1px solid rgba(92, 113, 109, 0.1);
  vertical-align: top;
}

th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: #f8f2e7;
}

.filter-row th {
  background: #fcfaf5;
}

.filter-row input {
  width: 100%;
}

.empty {
  text-align: center;
  color: #6b7e7a;
}

.filter-actions,
.pager,
.history-meta {
  display: flex;
  gap: 0.75rem;
  align-items: center;
}

.history-list {
  display: grid;
  gap: 0.8rem;
  margin-top: 1rem;
}

.history-item {
  padding: 0.9rem 1rem;
  border-radius: 16px;
  background: #ffffff;
  border: 1px solid rgba(96, 118, 115, 0.14);
  display: flex;
  justify-content: space-between;
  gap: 1rem;
}

.history-item.active {
  border-color: #c3914d;
}

.history-item p {
  margin: 0.2rem 0 0;
  color: #5f726d;
}

.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 2000;
  background: rgba(16, 24, 24, 0.82);
  display: grid;
  place-items: center;
  padding: 1.5rem;
}

.modal {
  width: min(1100px, 100%);
  max-height: calc(100vh - 3rem);
  padding: 1.5rem;
  display: grid;
  gap: 1rem;
  overflow: auto;
  background: #fffdf8;
  color: #1e2a2a;
  border: 1px solid rgba(69, 88, 84, 0.18);
  box-shadow: 0 30px 90px rgba(0, 0, 0, 0.28);
}

.ghost {
  background: #f5efe3;
}

.wizard-summary,
.wizard-body {
  display: grid;
  gap: 1rem;
}

.wizard-summary,
.note-card,
.form-grid {
  padding: 1rem;
  border-radius: 18px;
  background: #fff;
  border: 1px solid rgba(92, 113, 109, 0.12);
}

.summary-copy {
  margin: 0;
  font-size: 1rem;
  color: #314241;
}

.modal-header {
  align-items: start;
}

.modal-header .eyebrow,
.modal-header h3,
.wizard-summary .field span,
.note-card .label,
.field span {
  color: #4a5f5b;
}

.modal-header h3 {
  color: #152323;
}

.wizard-nav {
  padding: 0.9rem 1rem;
  border-radius: 16px;
  background: #eef4f2;
  color: #243332;
}

.warnings {
  padding: 0.9rem 1rem;
  background: #fff1d6;
  border: 1px solid rgba(198, 145, 77, 0.24);
  border-radius: 14px;
}

.warnings p,
.note-card p {
  margin: 0.35rem 0;
}

.note {
  white-space: pre-wrap;
  font-size: 1rem;
  line-height: 1.55;
  padding: 0.9rem 1rem;
  border-radius: 14px;
  background: #f7f2e9;
  color: #1f2d2d;
  border-left: 4px solid #c3914d;
}

.checkbox {
  display: flex;
  gap: 0.6rem;
  align-items: center;
  margin-top: 0.75rem;
  color: #233130;
}

.checkbox input {
  width: 18px;
  height: 18px;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.85rem;
  align-content: start;
}

.form-grid .field {
  padding: 0.85rem;
  border-radius: 14px;
  background: #fcfbf8;
  border: 1px solid rgba(92, 113, 109, 0.1);
}

.form-grid input {
  color: #1c2929;
  background: #fff;
}

.modal-footer {
  padding-top: 0.5rem;
  border-top: 1px solid rgba(92, 113, 109, 0.12);
}

@media (max-width: 960px) {
  .workspace {
    grid-template-columns: 1fr;
  }

  .sidebar {
    min-height: auto;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }

  .topbar,
  .preview-toolbar,
  .history-item,
  .modal-header,
  .modal-footer {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
