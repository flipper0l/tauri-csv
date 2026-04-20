<script setup lang="ts">
import type { NotesImportResult, ReviewedWizardItem } from "../types/workbook";

defineProps<{
  notesWizard: NotesImportResult;
  wizardItems: ReviewedWizardItem[];
  currentWizardItem: ReviewedWizardItem | null;
  wizardIndex: number;
  versionLabel: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  close: [];
  updateVersionLabel: [value: string];
  updateWizardIndex: [value: number];
  save: [];
}>();
</script>

<template>
  <div class="modal-backdrop">
    <section class="modal">
      <header class="modal-header">
        <div>
          <p class="eyebrow">Wizard note</p>
          <h3>Revisione record con commenti</h3>
        </div>
        <button type="button" class="ghost" @click="emit('close')">Chiudi</button>
      </header>

      <div class="wizard-summary">
        <p class="summary-copy">
          {{ notesWizard.matchedRows }} righe abbinate, {{ notesWizard.unmatchedRows }} non abbinate.
        </p>
        <label class="field">
          <span>Nome nuova versione</span>
          <input
            :value="versionLabel"
            type="text"
            @input="emit('updateVersionLabel', ($event.target as HTMLInputElement).value)"
          />
        </label>
        <div v-if="notesWizard.warnings.length > 0" class="warnings">
          <p v-for="warning in notesWizard.warnings" :key="warning">{{ warning }}</p>
        </div>
      </div>

      <div v-if="currentWizardItem" class="wizard-body">
        <div class="wizard-nav">
          <button
            type="button"
            :disabled="wizardIndex === 0"
            @click="emit('updateWizardIndex', Math.max(0, wizardIndex - 1))"
          >
            Prec.
          </button>
          <span>Record {{ wizardIndex + 1 }} / {{ wizardItems.length }}</span>
          <button
            type="button"
            :disabled="wizardIndex >= wizardItems.length - 1"
            @click="emit('updateWizardIndex', Math.min(wizardItems.length - 1, wizardIndex + 1))"
          >
            Succ.
          </button>
        </div>

        <div class="note-card">
          <p class="label">Foglio</p>
          <strong>{{ currentWizardItem.sheetName }}</strong>
          <p class="label">Nota ricevuta</p>
          <p class="note">{{ currentWizardItem.note }}</p>
          <label class="checkbox">
            <input v-model="currentWizardItem.apply" type="checkbox" />
            <span>Applica questa modifica alla nuova versione</span>
          </label>
        </div>

        <div class="form-grid">
          <label
            v-for="field in currentWizardItem.fields"
            :key="`${currentWizardItem.rowKey}-${field.column}`"
            class="field"
          >
            <span>{{ field.column }}</span>
            <input v-model="field.value" type="text" />
          </label>
        </div>
      </div>

      <footer class="modal-footer">
        <button type="button" class="ghost" @click="emit('close')">Annulla</button>
        <button type="button" :disabled="loading || wizardItems.length === 0" @click="emit('save')">
          Salva nuova versione
        </button>
      </footer>
    </section>
  </div>
</template>
