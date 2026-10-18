<script setup lang="ts">

import { computed, onMounted, ref } from 'vue';
import { t } from '@/i18n';
import { useResource } from '@/composables/useResource';
import type {
  NetworkAdapter,
  NetworkState,
  WifiNetwork,
} from '@/apps/nas-system-settings/types/network';
import wifiIcon from '@/assets/icons/network/wifi.svg';
import lanIcon from '@/assets/icons/network/lan.svg';
import lockIcon from '@/assets/icons/network/lock.svg';
import refreshIcon from '@/assets/icons/system/refresh.svg';

import SettingsTabLayout from '../SettingsTabLayout.vue';
import SettingsStatus from '../SettingsStatus.vue';
import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSkeleton from '@/components/commons/CommonSkeleton.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';

const { data, loading, error, busy, load, act } = useResource<NetworkState | null>(
  '/network',
  null,
);

const CONNECT_TIMEOUT = 60000;

const asking = ref<WifiNetwork | null>(null);
const password = ref('');

const scanning = ref(false);

const joining = ref('');
/** The network a connection was asked for while the cable is plugged in, to say why it cannot. */
const blocked = ref('');
/** Grows when a change of a card fails: its switch is drawn again in the state it really has. */
const switches = ref(0);
const connection = computed(() => data.value?.connection ?? null);

/**
 * Determines whether the network settings are editable.
 */
const editable = computed(() => Boolean(data.value?.canConnect));
const current = computed(() => (connection.value?.kind === 'wifi' ? connection.value.ssid : null));

/**
 * The details of the current connection as label-value pairs. Until the first reading the labels
 * are already there and the values are placeholders; the optional rows come with the reading.
 */
const details = computed(() => {
  const item = connection.value;

  return [
    { label: t('interface'), value: item?.interface },
    { label: t('ipAddress'), value: item?.ipv4 },
    { label: t('netmask'), value: item?.netmask },
    { label: t('gateway'), value: item?.gateway },
    { label: t('dns'), value: item?.dns.find((server) => !server.includes(':')) ?? item?.dns[0] },
    { label: t('macAddress'), value: item?.mac },
    ...(item?.speed ? [{ label: t('linkSpeed'), value: item.speed }] : []),
    ...(item && item.signal !== null ? [{ label: t('signal'), value: `${item.signal}%` }] : []),
    ...(item?.security ? [{ label: t('security'), value: item.security }] : []),
  ];
});

/**
 * Determines whether the given Wi-Fi network requires a password.
 * @param network The Wi-Fi network to check.
 * @returns True if the network requires a password, false otherwise.
 */
/**
 * What to do about a problem with the Wi-Fi: no adapter and no network found say only that, a
 * failure says how to look into it.
 */
function hint(problem: NetworkState['wifiProblem']) {
  return problem && problem !== 'noDevice' && problem !== 'disabled'
    ? t(`wifiProblemHint_${problem}`)
    : undefined;
}

function needsPassword(network: WifiNetwork) {
  return !network.known && network.security !== 'open';
}

/**
 * Starts the process of connecting to the given Wi-Fi network.
 * If the network requires a password, prompts the user for it.
 * @param network The Wi-Fi network to connect to.
 */
async function start(network: WifiNetwork) {
  // One way in at a time: on the cable the list is there to see, not to join.
  if (data.value?.wiredConnected) {
    blocked.value = network.ssid;
    asking.value = null;
    return;
  }
  blocked.value = '';
  if (needsPassword(network)) {
    asking.value = asking.value?.ssid === network.ssid ? null : network;
    password.value = '';
    return;
  }

  await connect(network);
}

/**
 * Connects to the specified Wi-Fi network.
 * @param network The Wi-Fi network to connect to.
 */
async function connect(network: WifiNetwork) {
  const body = needsPassword(network)
    ? { ssid: network.ssid, password: password.value }
    : { ssid: network.ssid };

  joining.value = network.ssid;
  try {
    if (await act('/network/wifi', 'POST', body, CONNECT_TIMEOUT)) {
      asking.value = null;
      password.value = '';
    }
  } finally {
    joining.value = '';
  }
}

/**
 * Leaves the wireless network.
 *
 * Offered only where it would not cut the launcher off: the backend refuses to disconnect the
 * interface the request arrived on, and does not set `canDisconnect` in the first place.
 */
async function leaveWifi() {
  joining.value = 'leave';
  try {
    await act('/network/disconnect', 'POST', {}, CONNECT_TIMEOUT);
  } finally {
    joining.value = '';
  }
}

async function useWired() {
  joining.value = 'wired';
  try {
    await act('/network/wired', 'POST', {}, CONNECT_TIMEOUT);
  } finally {
    joining.value = '';
  }
}

/** Turns a card off or back on; the backend never turns off the one in use. */
async function toggle(adapter: NetworkAdapter, enabled: boolean) {
  joining.value = `adapter_${adapter.interface}`;
  try {
    const body = { interface: adapter.interface, enabled };
    if (!(await act('/network/adapter', 'POST', body, CONNECT_TIMEOUT))) {
      switches.value += 1;
    }
  } finally {
    joining.value = '';
  }
}

/**
 * Scan for available Wi-Fi networks.
 */
async function scan() {
  scanning.value = true;
  try {
    await act('/network/scan', 'POST', {});
  } finally {
    scanning.value = false;
  }
}

// What NetworkManager remembers can be nothing at all: the page looks for itself on opening, and
// says it found nothing only once it has looked.
onMounted(scan);

</script>

<template>
  <SettingsTabLayout :title="t('networkSettings')" :error="!data ? error : ''" @retry="load">
    <p v-if="data && !editable" class="muted" data-testid="network_readonly">
      {{ t('networkReadOnly') }}
    </p>

    <section
      class="app_panel settings_group network_current"
      :aria-label="t('currentNetwork')"
      :aria-busy="!connection"
    >
      <div class="network_headline">
        <CommonIcon
          v-if="connection"
          :src="connection.kind === 'wifi' ? wifiIcon : lanIcon"
          size="large"
        />
        <CommonSkeleton v-else width="icon" />
        <div>
          <strong data-testid="network_name">
            <template v-if="connection">
              {{ connection.kind === 'wifi' ? (connection.ssid ?? t('wifi')) : t('wiredNetwork') }}
            </template>
            <CommonSkeleton v-else />
          </strong>
          <p class="muted">
            <template v-if="connection">
              {{ connection.connected ? t('connected') : t('disconnected') }}
            </template>
            <CommonSkeleton v-else width="short" />
          </p>
        </div>

        <button
          v-if="editable && connection?.kind === 'wifi' && data?.wiredAvailable"
          type="button"
          :disabled="busy"
          data-testid="network_use_wired"
          @click="useWired"
        >
          <CommonSpinner v-if="joining === 'wired'" size="small" compact />
          {{ t('useWired') }}
        </button>

        <!-- Leaving the wireless network, when it is not the one this page arrived through -->
        <button
          v-if="data?.canDisconnect"
          type="button"
          :disabled="busy"
          data-testid="network_disconnect"
          @click="leaveWifi"
        >
          <CommonSpinner v-if="joining === 'leave'" size="small" compact />
          {{ t('disconnectWifi') }}
        </button>
      </div>

      <dl class="network_details" data-testid="network_details">
        <div v-for="row in details" :key="row.label" class="network_detail">
          <dt>{{ row.label }}</dt>
          <dd>
            <template v-if="connection">{{ row.value }}</template>
            <CommonSkeleton v-else />
          </dd>
        </div>
      </dl>
    </section>

    <section
      v-if="data?.adapters.length"
      class="app_panel settings_group"
      :aria-label="t('networkAdapters')"
      data-testid="network_adapters"
    >
      <h2>{{ t('networkAdapters') }}</h2>
      <ul class="network_adapters">
        <li
          v-for="adapter in data.adapters"
          :key="adapter.interface"
          class="network_row network_line"
          :data-testid="`network_adapter_${adapter.interface}`"
        >
          <CommonIcon :src="adapter.kind === 'wifi' ? wifiIcon : lanIcon" />
          <span class="network_ssid">
            {{ adapter.kind === 'wifi' ? t('wifi') : t('wiredNetwork') }}
            <small class="muted">{{ adapter.interface }}</small>
          </span>
          <span v-if="adapter.inUse" class="network_badge network_badge_current">
            {{ t('adapterInUse') }}
          </span>
          <span class="muted">
            {{ adapter.enabled ? t(`adapterState_${adapter.state}`) : t('adapterOff') }}
          </span>
          <CommonSpinner v-if="joining === `adapter_${adapter.interface}`" size="small" compact />
          <CommonSwitch
            :key="`${adapter.interface}_${switches}`"
            :model-value="adapter.enabled"
            :label="t('adapterEnabled', { name: adapter.interface })"
            hide-label
            :disabled="!adapter.canToggle || busy"
            :title="adapter.inUse && adapter.enabled ? t('adapterInUseHint') : undefined"
            :testid="`network_adapter_toggle_${adapter.interface}`"
            @update:model-value="toggle(adapter, $event)"
          />
        </li>
      </ul>
    </section>

    <section
      class="app_panel settings_group"
      :aria-label="t('availableNetworks')"
      :aria-busy="!data"
    >
      <div class="app_toolbar network_heading">
        <h2>{{ t('availableNetworks') }}</h2>

        <button
          type="button"
          :disabled="!data || loading || busy"
          :aria-label="t('scan')"
          :title="t('scan')"
          data-testid="network_scan"
          @click="scan"
        >
          <CommonIcon :src="refreshIcon" />
        </button>
      </div>

      <SettingsStatus :error="data ? error : ''" />

      <!-- The same height whatever it holds: the panel does not shrink while it looks. -->
      <div class="network_results">
        <CommonSpinner
          v-if="!data || scanning"
          :label="t('scanningNetworks')"
          centered
          testid="network_scanning"
        />

        <CommonEmpty
          v-else-if="!data.wifi.length"
          :title="t(data.wifiProblem ? `wifiProblem_${data.wifiProblem}` : 'noNetworksFound')"
          :hint="hint(data.wifiProblem)"
          :icon="wifiIcon"
          testid="network_none"
        />

        <ul v-else class="app_scroll network_list">
          <li v-for="network in data.wifi" :key="network.ssid" class="network_row">
            <div class="network_line">
              <CommonIcon :src="wifiIcon" />
              <span class="network_ssid">
                {{ network.ssid }}
              </span>

              <CommonIcon
                v-if="network.security !== 'open'"
                :src="lockIcon"
                size="small"
                :label="t('security')"
              />

              <span v-if="network.known" class="network_badge">{{ t('knownNetwork') }}</span>
              <span v-if="current === network.ssid" class="network_badge network_badge_current">
                {{ t('connected') }}
              </span>

              <button
                v-else-if="editable"
                type="button"
                :disabled="busy"
                :aria-expanded="asking?.ssid === network.ssid"
                :data-testid="`network_connect_${network.ssid}`"
                @click="start(network)"
              >
                <CommonSpinner v-if="joining === network.ssid" size="small" compact />
                {{ t(joining === network.ssid ? 'connecting' : 'connect') }}
              </button>
            </div>

            <output
              v-if="blocked === network.ssid"
              class="muted network_note"
              :data-testid="`network_blocked_${network.ssid}`"
            >
              {{ t('wifiWhileWired') }}
            </output>

            <form
              v-if="asking?.ssid === network.ssid"
              class="network_connect"
              @submit.prevent="connect(network)"
            >
              <label class="settings_field">
                {{ t('networkPassword') }}

                <!-- The network the password is for, the field the browser wants beside one. -->
                <input
                  class="visually_hidden"
                  type="text"
                  name="network_ssid"
                  autocomplete="username"
                  :value="network.ssid"
                  readonly
                  tabindex="-1"
                  aria-hidden="true"
                />
                <input
                  v-model="password"
                  name="network_password"
                  type="password"
                  autocomplete="off"
                  minlength="8"
                  maxlength="64"
                  required
                  data-testid="network_password"
                />
              </label>

              <button
                type="submit"
                class="primary"
                :disabled="busy"
                data-testid="network_connect_submit"
              >
                {{ t('connect') }}
              </button>
            </form>
          </li>
        </ul>
      </div>
    </section>
  </SettingsTabLayout>
</template>

<style scoped lang="scss">

.network_headline {
  @extend %fx_base;
  gap: $space_3;

  p {
    margin: 0;
  }

  button {
    margin-left: auto;
  }
}

.network_details {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax($metric_min_width * 1.5, 1fr));
  gap: $space_3;
  margin: 0;
}

.network_detail {
  @extend %info_tile;
}

.network_heading h2 {
  margin-right: auto;
}

.network_results {
  display: flex;
  flex-direction: column;
  justify-content: center;
  height: $network_list_height;
}

.network_list {
  @extend %stack;
  flex: 1;
  gap: $space_2;
  min-height: 0;
}

.network_row {
  @extend %panel_sunken;
  padding: $space_2 $space_3;
}

.network_line {
  @extend %fx_base;
  gap: $space_3;
}

.network_ssid {
  @extend %truncated_text;
  margin-right: auto;
}

.network_badge {
  @extend %badge;
}

.network_badge_current {
  border-color: var(--accent);
  color: var(--accent);
}

.network_adapters {
  @extend %stack;
  gap: $space_2;
  margin: 0;
  padding: 0;
  list-style: none;

  small {
    margin-left: $space_2;
  }
}

.network_note {
  margin: 0;
  padding-top: $space_2;
}

.network_connect {
  @extend %fx_base;
  gap: $space_3;
  padding-top: $space_3;
}

</style>
