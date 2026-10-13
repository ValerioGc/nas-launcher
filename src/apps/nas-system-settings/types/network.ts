/**
 * Represents a network connection, including its type, status, and configuration details.
 */
export interface NetworkConnection {
  kind: 'lan' | 'wifi';
  connected: boolean;
  interface: string;
  ssid: string | null;
  signal: number | null;
  security: string | null;
  ipv4: string;
  netmask: string;
  gateway: string;
  dns: string[];
  mac: string;
  speed: string | null;
}

/**
 * Represents a Wi-Fi network available for connection.
 */
export interface WifiNetwork {
  ssid: string;
  signal: number;
  security: string;
  known: boolean;
}

/**
 * Represents the current state of the network, including the active connection, available Wi-Fi networks.
 */
export interface NetworkState {
  connection: NetworkConnection;
  wifi: WifiNetwork[];
  wiredAvailable: boolean;
  canConnect: boolean;
  canDisconnect: boolean;
  wifiProblem: WifiProblem | null;
}

/**
 * Why no wireless network could be listed: NetworkManager does not answer, it knows no Wi-Fi
 * device, or the device gave no list.
 */
export type WifiProblem = 'noNetworkManager' | 'noDevice' | 'listFailed';
