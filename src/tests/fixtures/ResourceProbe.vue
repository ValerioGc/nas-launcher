<script setup lang="ts">

import { useResource } from '@/composables/useResource';
const { data, loading, error, busy, load, act } = useResource<string[]>('/probe', []);

</script>
<template>
  <div>
    <p>{{ data.join(',') }}</p>
    <span>{{ loading ? 'loading' : '' }} {{ error }}</span
    ><button type="button" @click="load">refresh</button
    ><button type="button" :disabled="busy" @click="act('/probe', 'POST')">save</button>
  </div>
</template>
