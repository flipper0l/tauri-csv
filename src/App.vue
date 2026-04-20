<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
import * as XLSX from "xlsx";
import AppSidebar from "./components/AppSidebar.vue";
import AppTopbar from "./components/AppTopbar.vue";
import NotesWizard from "./components/NotesWizard.vue";
import PreviewTable from "./components/PreviewTable.vue";
import StatusMessages from "./components/StatusMessages.vue";
import "./styles/app.css";
import type {
  ColumnFilter,
  ExportResult,
  NotesImportResult,
  ProjectView,
  ReviewedWizardItem,
  SheetPreview,
  WorkbookSelection,
} from "./types/workbook";
import { workbookNameFromPath, workbookToSelection } from "./utils/workbook";

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
let unlistenWipeEvent: (() => void) | null = null;

const totalPages = computed(() => {
  if (!preview.value) {
    return 1;
  }
  return Math.max(1, Math.ceil(preview.value.total / preview.value.pageSize));
});

const currentWizardItem = computed(() => wizardItems.value[wizardIndex.value] ?? null);
const activeVersion = computed(() =>
  project.value?.versions.find((version) => version.id === selectedVersionId.value) ?? null,
);

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
  selectedVersionId.value = imported.activeVersionId;
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
      projectId: project.value.projectId,
      versionId: selectedVersionId.value,
      sheetName: selectedSheet.value,
      sheet: resolveReturnedSheet(selection),
    },
  });

  notesWizard.value = result;
  wizardItems.value = result.reviewItems.map((item) => ({
    ...item,
    apply: true,
    fields: item.fields.map((field) => ({ ...field })),
  }));
  wizardIndex.value = 0;
  versionLabel.value = `Versione ${project.value.versions.length + 1}`;
  successMessage.value = `Trovate ${result.totalRowsWithNotes} righe con note. ${result.matchedRows} pronte per la revisione.`;
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
  selectedVersionId.value = current.activeVersionId;
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
        projectId: project.value.projectId,
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

function resetWorkspaceState() {
  project.value = null;
  selectedSheet.value = "";
  selectedVersionId.value = "";
  preview.value = null;
  notesWizard.value = null;
  wizardItems.value = [];
  wizardIndex.value = 0;
  versionLabel.value = "";
  page.value = 1;
  syncFilters([]);
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
    const result = await invoke<{ project: ProjectView; createdVersionId: string }>("apply_notes_import", {
      payload: {
        projectId: project.value.projectId,
        baseVersionId: notesWizard.value.baseVersionId,
        label: versionLabel.value,
        reviewedItems: wizardItems.value,
      },
    });

    project.value = result.project;
    selectedVersionId.value = result.createdVersionId;
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
        projectId: project.value.projectId,
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
      XLSX.writeFile(workbook, `${result.workbookName}-${result.versionLabel}.xlsx`);
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
  if (preview.value && preview.value.sheetName !== selectedSheet.value) {
    preview.value = null;
  }
});

onMounted(async () => {
  if (isTauriRuntime()) {
    unlistenWipeEvent = await listen<string | null>("app-data-wiped", (event) => {
      busyLabel.value = "";
      loading.value = false;

      if (event.payload) {
        errorMessage.value = event.payload;
        return;
      }

      resetFeedback();
      resetWorkspaceState();
      successMessage.value = "Tutti i dati importati sono stati cancellati.";
    });
  }

  await loadCurrentProject();
});

onBeforeUnmount(() => {
  unlistenWipeEvent?.();
  unlistenWipeEvent = null;
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

    <AppSidebar
      :project="project"
      :selected-sheet="selectedSheet"
      :loading="loading"
      @select-sheet="changeSheet"
      @import-original="importOriginalWorkbook"
    />

    <section class="main-panel">
      <AppTopbar
        :project="project"
        :selected-sheet="selectedSheet"
        :selected-version-id="selectedVersionId"
        :loading="loading"
        @change-version="changeVersion"
        @import-notes="importNotesWorkbook"
        @export-version="exportCurrentVersion"
      />

      <StatusMessages
        :busy-label="busyLabel"
        :success-message="successMessage"
        :error-message="errorMessage"
      />

      <PreviewTable
        v-if="project && preview"
        :preview="preview"
        :loading="loading"
        :page="page"
        :total-pages="totalPages"
        :filters="filters"
        @apply-filters="applyFilters"
        @go-to-page="goToPage"
      />

      <section v-else class="empty-state">
        <h3>Importa un workbook per iniziare</h3>
        <p>
          L'app creerà uno storico delle versioni, mostrerà i fogli nella sidebar e ti permetterà di
          revisionare le note ricevute con un wizard dedicato.
        </p>
      </section>
    </section>

    <NotesWizard
      v-if="notesWizard"
      :notes-wizard="notesWizard"
      :wizard-items="wizardItems"
      :current-wizard-item="currentWizardItem"
      :wizard-index="wizardIndex"
      :version-label="versionLabel"
      :loading="loading"
      @close="closeWizard"
      @update-version-label="versionLabel = $event"
      @update-wizard-index="wizardIndex = $event"
      @save="saveReviewedVersion"
    />
  </main>
</template>
