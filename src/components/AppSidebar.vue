<script setup lang="ts">
import { formatStamp } from "../utils/format";
import type { ProjectView } from "../types/workbook";

defineProps<{
  project: ProjectView | null;
  selectedSheet: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  selectSheet: [sheetName: string];
  importOriginal: [];
}>();
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <p class="eyebrow">Dematerializzazione</p>
      <h1>Versiona, filtra e integra note.</h1>
    </div>

    <section v-if="project" class="project-card">
      <p class="label">Workbook attivo</p>
      <strong>{{ project.name }}</strong>
      <p class="meta">Importato il {{ formatStamp(project.importedAt) }}</p>
    </section>

    <section v-if="project" class="sheet-nav">
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
        @click="emit('selectSheet', sheet.name)"
      >
        <span>{{ sheet.name }}</span>
        <small>{{ sheet.rows }} righe</small>
      </button>
    </section>

    <button type="button" class="import-button" :disabled="loading" @click="emit('importOriginal')">
      Import
    </button>
  </aside>
</template>
