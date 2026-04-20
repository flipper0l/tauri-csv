<script setup lang="ts">
import type { SheetPreview } from "../types/workbook";

defineProps<{
  preview: SheetPreview;
  loading: boolean;
  page: number;
  totalPages: number;
  filters: Record<string, string>;
}>();

const emit = defineEmits<{
  applyFilters: [];
  goToPage: [page: number];
}>();
</script>

<template>
  <section class="preview-card">
    <div class="preview-toolbar">
      <div>
        <strong>{{ preview.total }}</strong>
        <span> righe trovate con i filtri attivi</span>
      </div>
      <div class="pager">
        <button type="button" :disabled="page <= 1 || loading" @click="emit('goToPage', page - 1)">Prec.</button>
        <span>Pagina {{ page }} / {{ totalPages }}</span>
        <button type="button" :disabled="page >= totalPages || loading" @click="emit('goToPage', page + 1)">Succ.</button>
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
                @keyup.enter="emit('applyFilters')"
              />
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="preview.rows.length === 0">
            <td :colspan="preview.columns.length" class="empty">Nessun record trovato.</td>
          </tr>
          <tr v-for="row in preview.rows" :key="row.key">
            <td v-for="(cell, index) in row.cells" :key="`${row.key}-${index}`">{{ cell }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
