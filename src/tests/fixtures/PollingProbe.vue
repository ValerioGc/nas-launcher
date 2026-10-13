<script setup lang="ts">

import { ref } from 'vue';
import { usePolling } from '@/composables/usePolling';

interface Props {
  action: () => void;
}

const props = defineProps<Props>();
const interval = ref(1000);

usePolling(props.action, interval);

defineExpose({ interval });

</script>
<template><span>polling</span></template>
