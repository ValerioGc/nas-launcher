<script lang="ts" setup>

import { t } from '@/i18n';
import { computed, ref, watch } from 'vue';
import { appIcon, appInfo, appLabel } from '@/apps/registry';

import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';

import type { AppDetail } from '@/apps/nas-app-manager/types';
import { request } from '@/api/client';

function repositoryName(value: string) {
  return new URL(value).pathname.split('/').filter(Boolean).join('/');
}

function safeLink(value: string) {
  try {
    const url = new URL(value);
    return ['https:', 'http:'].includes(url.protocol) ? url.href : undefined;
  } catch {
    return undefined;
  }
}

const props = defineProps<{
  appId: string | null;
}>();

const open = defineModel<boolean>({ required: true });

const detail = ref<AppDetail | null>(null);
const loading = ref(false);
const errorMessage = ref('');
const name = computed(() => appLabel(props.appId ?? '', detail.value?.name ?? ''));
const description = computed(() => appInfo(props.appId ?? '', detail.value?.description ?? ''));

async function fetchDetail(id: string) {
  loading.value = true;
  errorMessage.value = '';
  detail.value = null;
  try {
    detail.value = await request<AppDetail>(`/apps/${encodeURIComponent(id)}/info`);
  } catch {
    errorMessage.value = t('appDetailsFailed');
  } finally {
    loading.value = false;
  }
}

watch(open, (isOpen) => {
  if (isOpen && props.appId) {
    fetchDetail(props.appId);
  }
});

</script>

<template>
  <CommonModal v-model="open" :title="t('appDetails')" wide>
    <CommonSpinner v-if="loading" testid="detail_modal_loading" />

    <p v-else-if="errorMessage" role="alert" data-testid="detail_modal_error">
      {{ errorMessage }}
    </p>

    <div v-else-if="detail" class="detail_body" data-testid="detail_modal_body">
      <img class="detail_logo" :src="appIcon(props.appId ?? '', detail.logo)" :alt="name" />
      <div class="detail_information">
        <h3>{{ name }}</h3>
        <p v-if="description" class="muted">{{ description }}</p>

        <dl class="detail_facts">
          <div>
            <dt>{{ t('version') }}</dt>
            <dd>{{ detail.version || t('notInstalled') }}</dd>
          </div>
          <div v-if="safeLink(detail.url_repo)">
            <dt>{{ t('repository') }}</dt>
            <dd>
              <a :href="safeLink(detail.url_repo)" target="_blank" rel="noopener noreferrer">{{
                repositoryName(detail.url_repo)
              }}</a>
            </dd>
          </div>
          <div v-if="safeLink(detail.website)">
            <dt>{{ t('website') }}</dt>
            <dd>
              <a :href="safeLink(detail.website)" target="_blank" rel="noopener noreferrer">{{
                detail.website
              }}</a>
            </dd>
          </div>
        </dl>
      </div>
    </div>

    <p v-else>{{ t('noAppSelected') }}</p>
  </CommonModal>
</template>

<style scoped lang="scss">

.detail_body {
  display: grid;
  grid-template-columns: 6rem minmax(0, 1fr);
  align-items: start;
  gap: $space_5;

  .detail_logo {
    width: 6rem;
    height: 6rem;
    object-fit: contain;
    border-radius: $radius_control;
  }

  .detail_information {
    min-width: 0;
    @extend %stack;
    gap: $space_3;
  }

  h3,
  p {
    margin: 0;
  }
}

.detail_facts {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: $space_2 $space_4;
  margin: 0;

  div {
    display: contents;
  }

  dt {
    color: var(--surface-muted);
  }

  dd {
    @extend %truncated_text;
    margin: 0;
  }
}

@media (max-width: 36rem) {
  .detail_body {
    grid-template-columns: 1fr;

    .detail_logo {
      width: 5rem;
      height: 5rem;
    }
  }
}

</style>
