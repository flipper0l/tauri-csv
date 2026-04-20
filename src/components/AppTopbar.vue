<script setup lang="ts">
import type { ProjectView } from "../types/workbook";

defineProps<{
  project: ProjectView | null;
  selectedSheet: string;
  selectedVersionId: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  changeVersion: [versionId: string];
  importNotes: [];
  exportVersion: [];
}>();
</script>

<template>
  <header class="topbar">
    <div>
      <p class="eyebrow">Centro operativo</p>
      <h2>{{ selectedSheet || "Nessun foglio selezionato" }}</h2>
    </div>

    <div v-if="project" class="actions">
      <label class="field compact">
        <span>Versione</span>
        <select :value="selectedVersionId" @change="emit('changeVersion', ($event.target as HTMLSelectElement).value)">
          <option v-for="version in project.versions" :key="version.id" :value="version.id">
            {{ version.label }}
          </option>
        </select>
      </label>

      <button type="button" :disabled="loading" @click="emit('importNotes')">
        Importa file con note
      </button>
      <button type="button" :disabled="loading" @click="emit('exportVersion')">
        Esporta versione
      </button>
    </div>
  </header>
</template>
