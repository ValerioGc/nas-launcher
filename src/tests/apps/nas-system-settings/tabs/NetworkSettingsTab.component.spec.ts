import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import NetworkSettingsTab from '@/apps/nas-system-settings/components/tabs/NetworkSettingsTab.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const live = {
  connection: {
    kind: 'wifi',
    connected: true,
    interface: 'wlp1s0',
    ssid: 'Casa',
    signal: 55,
    security: 'WPA2',
    ipv4: '192.168.1.10',
    netmask: '255.255.255.0',
    gateway: '192.168.1.1',
    dns: ['192.168.1.1'],
    mac: 'd8:3a:dd:2f:91:c4',
    speed: null,
  },
  wifi: [
    { ssid: 'Casa', signal: 86, security: 'WPA2', known: true },
    { ssid: 'Studio', signal: 60, security: 'WPA2', known: true },
    { ssid: 'Ospiti', signal: 40, security: 'WPA2', known: false },
  ],
  wiredAvailable: true,
  wiredConnected: false,
  adapters: [
    {
      interface: 'wlp1s0',
      kind: 'wifi',
      state: 'connected',
      enabled: true,
      inUse: true,
      canToggle: false,
    },
    {
      interface: 'enp2s0',
      kind: 'lan',
      state: 'unavailable',
      enabled: true,
      inUse: false,
      canToggle: true,
    },
  ],
  canConnect: true,
  canDisconnect: false,
  wifiProblem: null,
};

const readOnly = { ...live, canConnect: false };

beforeEach(() => {
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue(live);
});

describe('network settings', () => {
  it('shows placeholders for the connection and a spinner for the networks while reading', async () => {
    let answer!: (value: typeof live) => void;
    vi.mocked(request).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)));
    const wrapper = mount(NetworkSettingsTab);

    expect(wrapper.find('[data-testid="settings_loading"]').exists()).toBe(false);
    const current = wrapper.get('[aria-label="Connessione attuale"]');
    const networks = wrapper.get('[aria-label="Reti disponibili"]');
    expect(current.attributes('aria-busy')).toBe('true');
    expect(current.get('[data-testid="network_details"]').text()).toContain('Indirizzo IP');
    expect(current.findAll('[data-testid="common_skeleton"]').length).toBeGreaterThan(6);
    expect(networks.get('[data-testid="network_scanning"]').text()).toContain('Ricerca delle reti');
    expect(networks.find('[data-testid="network_none"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="network_scan"]').attributes('disabled')).toBeDefined();

    answer(live);
    await flushPromises();
    expect(wrapper.findAll('[data-testid="common_skeleton"]')).toHaveLength(0);
    expect(current.attributes('aria-busy')).toBe('false');
    expect(wrapper.get('[data-testid="network_name"]').text()).toBe('Casa');
    wrapper.unmount();
  });

  it('shows the first IPv4 server of the DNS on a line of its own', async () => {
    vi.mocked(request).mockResolvedValue({
      ...live,
      connection: { ...live.connection, dns: ['fe80::1%wlp1s0', '192.168.1.1', '1.1.1.1'] },
    });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    const row = wrapper
      .findAll('[data-testid="network_details"] div')
      .find((item) => item.text().startsWith('DNS'));
    expect(row?.text()).toBe('DNS192.168.1.1');
    wrapper.unmount();
  });

  it('shows the link of the machine, with what only a real one has', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    expect(wrapper.get('[data-testid="network_name"]').text()).toBe('Casa');
    const details = wrapper.get('[data-testid="network_details"]').text();
    expect(details).toContain('wlp1s0');
    expect(details).toContain('192.168.1.10');
    expect(details).toContain('Segnale');
    expect(details).toContain('55%');
    expect(details).toContain('WPA2');
    expect(details).not.toContain('Velocità');
    wrapper.unmount();
  });

  it('asks for a password only on a network never used before', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    expect(wrapper.find('[data-testid="network_connect_Casa"]').exists()).toBe(false);
    await wrapper.get('[data-testid="network_connect_Studio"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('[data-testid="network_password"]').exists()).toBe(false);
    expect(request).toHaveBeenCalledWith(
      '/network/wifi',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ ssid: 'Studio' }) }),
    );

    await wrapper.get('[data-testid="network_connect_Ospiti"]').trigger('click');
    await wrapper.get('[data-testid="network_password"]').setValue('parolalunga');
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/network/wifi',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ ssid: 'Ospiti', password: 'parolalunga' }),
      }),
    );
    wrapper.unmount();
  });

  it('asks the antenna to look again instead of re-reading what it knows', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    await wrapper.get('[data-testid="network_scan"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/network/scan',
      expect.objectContaining({ method: 'POST' }),
    );
    wrapper.unmount();
  });

  it('shows the reason the machine gives when it refuses', async () => {
    vi.mocked(request).mockImplementation(async (path, options) => {
      if (options?.method === 'POST') {
        throw new Error('the network refused the password');
      }

      return live;
    });

    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    await wrapper.get('[data-testid="network_connect_Ospiti"]').trigger('click');
    await wrapper.get('[data-testid="network_password"]').setValue('parolalunga');
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(wrapper.text()).toContain('the network refused the password');
    wrapper.unmount();
  });

  it('hides every action that would change the network where it is not allowed', async () => {
    vi.mocked(request).mockResolvedValue(readOnly);
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    expect(wrapper.get('[data-testid="network_readonly"]').text()).toContain(
      'Modifica della rete non disponibile',
    );
    expect(wrapper.find('[data-testid="network_connect_Ospiti"]').exists()).toBe(false);
    expect(wrapper.findAll('button').some((b) => b.text() === 'Usa la rete via cavo')).toBe(false);
    await wrapper.get('[data-testid="network_scan"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/network/scan',
      expect.objectContaining({ method: 'POST' }),
    );
    wrapper.unmount();
  });

  it('goes back to the cable when the machine has one', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    await wrapper.get('[data-testid="network_use_wired"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith(
      '/network/wired',
      expect.objectContaining({ method: 'POST', body: '{}' }),
    );
    wrapper.unmount();
  });

  it('does not offer the cable where there is none', async () => {
    vi.mocked(request).mockResolvedValue({ ...live, wiredAvailable: false });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    expect(wrapper.find('[data-testid="network_use_wired"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('closes the password row when the same network is clicked twice', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    await wrapper.get('[data-testid="network_connect_Ospiti"]').trigger('click');
    expect(wrapper.find('[data-testid="network_password"]').exists()).toBe(true);

    await wrapper.get('[data-testid="network_connect_Ospiti"]').trigger('click');
    expect(wrapper.find('[data-testid="network_password"]').exists()).toBe(false);
    expect(request).not.toHaveBeenCalledWith('/network/wifi', expect.anything());
    wrapper.unmount();
  });

  it('lists the cards, keeps the one in use on and turns another one off', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    const cards = wrapper.get('[data-testid="network_adapters"]');
    expect(cards.get('h2').text()).toBe('Schede di rete');

    const wifi = wrapper.get('[data-testid="network_adapter_wlp1s0"]');
    expect(wifi.text()).toContain('In uso');
    expect(wifi.text()).toContain('Connessa');
    const inUse = wrapper.get('[data-testid="network_adapter_toggle_wlp1s0"]');
    expect(inUse.attributes('title')).toBe('La scheda in uso non può essere disattivata.');
    expect(inUse.get('input').attributes('disabled')).toBeDefined();

    const wired = wrapper.get('[data-testid="network_adapter_enp2s0"]');
    expect(wired.text()).toContain('Rete via cavo');
    expect(wired.text()).toContain('Non disponibile');
    await wrapper.get('[data-testid="network_adapter_toggle_enp2s0_input"]').setValue(false);
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/network/adapter',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ interface: 'enp2s0', enabled: false }),
      }),
    );
    wrapper.unmount();
  });

  it('draws a card again as it is when turning it off fails, and says it is off', async () => {
    vi.mocked(request).mockImplementation(async (path, options) => {
      if (options?.method === 'POST' && path === '/network/adapter') {
        throw new Error('this card cannot be turned off');
      }
      return path === '/network'
        ? live
        : { ...live, adapters: [{ ...live.adapters[1], enabled: false }] };
    });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    const toggle = () =>
      wrapper.get<HTMLInputElement>('[data-testid="network_adapter_toggle_enp2s0_input"]');
    await toggle().setValue(false);
    await flushPromises();
    expect(wrapper.text()).toContain('this card cannot be turned off');
    expect(toggle().element.checked).toBe(true);

    vi.mocked(request).mockResolvedValue({
      ...live,
      adapters: [{ ...live.adapters[1], enabled: false }],
      wifi: [],
      wifiProblem: 'disabled',
    });
    await wrapper.get('[data-testid="network_scan"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[data-testid="network_adapter_enp2s0"]').text()).toContain('Disattivata');
    expect(wrapper.get('[data-testid="network_none"]').text()).toBe('Scheda Wi-Fi disattivata');
    wrapper.unmount();
  });

  it('shows the networks with the cable plugged in, and says why it joins none', async () => {
    vi.mocked(request).mockResolvedValue({ ...live, wiredConnected: true });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    await wrapper.get('[data-testid="network_connect_Ospiti"]').trigger('click');
    expect(wrapper.get('[data-testid="network_blocked_Ospiti"]').text()).toBe(
      'Con il cavo di rete collegato non è possibile collegarsi anche al Wi-Fi.',
    );
    expect(wrapper.find('[data-testid="network_password"]').exists()).toBe(false);
    await wrapper.get('[data-testid="network_connect_Studio"]').trigger('click');
    expect(wrapper.find('[data-testid="network_blocked_Ospiti"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="network_blocked_Studio"]').exists()).toBe(true);
    expect(request).not.toHaveBeenCalledWith('/network/wifi', expect.anything());
    wrapper.unmount();
  });

  it('says only that no network was found', async () => {
    vi.mocked(request).mockResolvedValue({ ...live, wifi: [] });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    expect(wrapper.get('[data-testid="network_none"]').text()).toBe('Nessuna rete trovata');
    wrapper.unmount();
  });

  it('says only that no adapter was detected, instead of no network found', async () => {
    vi.mocked(request).mockResolvedValue({ ...live, wifi: [], wifiProblem: 'noDevice' });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    expect(wrapper.get('[data-testid="network_none"]').text()).toBe(
      'Nessuna scheda di rete rilevata',
    );
    wrapper.unmount();
  });

  it('says how to look into a search that failed', async () => {
    vi.mocked(request).mockResolvedValue({ ...live, wifi: [], wifiProblem: 'noNetworkManager' });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    const empty = wrapper.get('[data-testid="network_none"]');
    expect(empty.text()).toContain('Servizio di rete non disponibile.');
    expect(empty.text()).not.toContain('systemctl');
    wrapper.unmount();
  });

  it('names a wired link after the cable, not after a network', async () => {
    vi.mocked(request).mockResolvedValue({
      ...live,
      connection: { ...live.connection, kind: 'lan', ssid: null, signal: null, speed: '1 Gb/s' },
    });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    expect(wrapper.get('[data-testid="network_name"]').text()).toBe('Rete via cavo');
    expect(wrapper.get('[data-testid="network_details"]').text()).toContain('1 Gb/s');
    wrapper.unmount();
  });

  it('looks for networks on opening and says it is looking while the scan runs', async () => {
    let release: (value: unknown) => void = () => {};
    vi.mocked(request).mockImplementation(async (_path, options) => {
      if (options?.method === 'POST') {
        await new Promise((resolve) => {
          release = resolve;
        });
      }
      return { ...live, wifi: [] };
    });
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();

    expect(request).toHaveBeenCalledWith(
      '/network/scan',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(wrapper.get('[data-testid="network_scanning"]').text()).toContain('Ricerca delle reti');
    expect(wrapper.find('[data-testid="network_none"]').exists()).toBe(false);

    release(undefined);
    await flushPromises();
    expect(wrapper.find('[data-testid="network_scanning"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="network_none"]').text()).toContain('Nessuna rete');

    wrapper.get('[data-testid="network_scan"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('[data-testid="network_scanning"]').exists()).toBe(true);
    release(undefined);
    await flushPromises();
    wrapper.unmount();
  });

  it('offers to leave the wireless network only when that would not cut the launcher off', async () => {
    const wrapper = mount(NetworkSettingsTab);
    await flushPromises();
    expect(wrapper.find('[data-testid="network_disconnect"]').exists()).toBe(false);
    wrapper.unmount();

    vi.mocked(request).mockResolvedValue({ ...live, canDisconnect: true });
    const other = mount(NetworkSettingsTab);
    await flushPromises();

    await other.get('[data-testid="network_disconnect"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith(
      '/network/disconnect',
      expect.objectContaining({ method: 'POST', body: '{}' }),
    );
    other.unmount();
  });
});
