<script setup lang="ts">

import { ref } from 'vue';
const value = ref('');

</script>
<template>
  <label>Draft<input v-model="value" /></label>
</template>
