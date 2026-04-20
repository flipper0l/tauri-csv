<script setup lang="ts">
defineProps<{
  busyLabel: string;
  successMessage: string;
  errorMessage: string;
}>();
</script>

<template>
  <p v-if="busyLabel" class="status info">{{ busyLabel }}...</p>
  <p v-if="successMessage" class="status success">{{ successMessage }}</p>
  <p v-if="errorMessage" class="status error">{{ errorMessage }}</p>
</template>
