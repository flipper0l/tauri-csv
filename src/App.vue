<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type ItemRow = {
  ID: number;
  name: string;
  surname: string;
};

type PageResult = {
  items: ItemRow[];
  total: number;
  page: number;
  page_size: number;
};

const selectedPath = ref("");
const loading = ref(false);
const errorMessage = ref("");
const successMessage = ref("");

const rows = ref<ItemRow[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = ref(10);

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize.value)));

async function chooseCsvFile() {
  errorMessage.value = "";
  successMessage.value = "";

  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });

  if (typeof selected === "string") {
    selectedPath.value = selected;
  }
}

async function fetchPage(targetPage: number) {
  const data = await invoke<PageResult>("get_table_page", {
    page: targetPage,
    page_size: pageSize.value,
  });

  rows.value = data.items;
  total.value = data.total;
  page.value = data.page;
}

async function importCsv() {
  if (!selectedPath.value) {
    errorMessage.value = "Please select a CSV file first.";
    return;
  }

  loading.value = true;
  errorMessage.value = "";
  successMessage.value = "";

  try {
    const result = await invoke<{ inserted_rows: number }>("import_csv_to_memory_db", {
      payload: { path: selectedPath.value },
    });

    await fetchPage(1);
    successMessage.value = `Imported ${result.inserted_rows} row(s) into ATable.`;
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function prevPage() {
  if (page.value > 1) {
    await fetchPage(page.value - 1);
  }
}

async function nextPage() {
  if (page.value < totalPages.value) {
    await fetchPage(page.value + 1);
  }
}
</script>

<template>
  <main class="app-shell">
    <section class="card">
      <h1>CSV Loader</h1>
      <p class="subtitle">Select a local CSV file and load data into an in-memory SQLite table named ATable.</p>

      <div class="controls">
        <button type="button" @click="chooseCsvFile" :disabled="loading">Select CSV</button>
        <button type="button" @click="importCsv" :disabled="loading || !selectedPath">Load to SQLite</button>
      </div>

      <p class="path" v-if="selectedPath">Selected: {{ selectedPath }}</p>
      <p class="feedback success" v-if="successMessage">{{ successMessage }}</p>
      <p class="feedback error" v-if="errorMessage">{{ errorMessage }}</p>
    </section>

    <section class="card table-card">
      <div class="table-header">
        <h2>ATable</h2>
        <p>Total rows: {{ total }}</p>
      </div>

      <div class="table-wrapper">
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>name</th>
              <th>surname</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="rows.length === 0">
              <td colspan="3" class="empty">No data loaded yet.</td>
            </tr>
            <tr v-for="item in rows" :key="item.ID">
              <td>{{ item.ID }}</td>
              <td>{{ item.name }}</td>
              <td>{{ item.surname }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="pagination">
        <button type="button" @click="prevPage" :disabled="page <= 1 || loading">Previous</button>
        <span>Page {{ page }} / {{ totalPages }}</span>
        <button type="button" @click="nextPage" :disabled="page >= totalPages || loading">Next</button>
      </div>
    </section>
  </main>
</template>

<style scoped>
:global(body) {
  margin: 0;
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  background: linear-gradient(165deg, #f8f1e7 0%, #f1f8f6 100%);
  color: #222;
}

.app-shell {
  max-width: 920px;
  margin: 0 auto;
  padding: 2rem 1rem;
  display: grid;
  gap: 1rem;
}

.card {
  background: #fffdf8;
  border: 1px solid #eadfcd;
  border-radius: 14px;
  padding: 1rem;
  box-shadow: 0 8px 20px rgba(62, 50, 25, 0.08);
}

h1,
h2 {
  margin: 0;
}

.subtitle {
  margin-top: 0.5rem;
}

.controls {
  margin-top: 1rem;
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
}

button {
  border: 1px solid #bb9e71;
  border-radius: 8px;
  padding: 0.5rem 0.9rem;
  background: #fff;
  color: #2d2419;
  font-weight: 600;
  cursor: pointer;
}

button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.path {
  margin-top: 0.75rem;
  word-break: break-all;
}

.feedback {
  margin-top: 0.5rem;
}

.success {
  color: #1d6e52;
}

.error {
  color: #b44335;
}

.table-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 1rem;
}

.table-wrapper {
  overflow-x: auto;
  margin-top: 0.75rem;
}

table {
  width: 100%;
  border-collapse: collapse;
}

th,
td {
  text-align: left;
  padding: 0.6rem;
  border-bottom: 1px solid #eadfcd;
}

.empty {
  text-align: center;
  color: #7d6f57;
}

.pagination {
  margin-top: 0.9rem;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1rem;
}

@media (max-width: 700px) {
  .table-header {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>