import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Activity,
  Cable,
  CircleStop,
  Network,
  Play,
  Plus,
  Radio,
  RefreshCw,
  Route,
  Send,
  Settings,
  SquareTerminal,
  Unplug,
  Usb
} from 'lucide-react';

type Tab = 'midi' | 'routing' | 'monitor' | 'ndi' | 'settings';

type MidiDevice = {
  id: string;
  name: string;
  direction: 'input' | 'output';
  index: number;
};

type MidiSnapshot = { inputs: MidiDevice[]; outputs: MidiDevice[] };
type MidiEvent = { timestamp: number; source: string; bytes: number[] };
type NdiSource = { name: string; url?: string | null };
type NdiStatus = { available: boolean; library?: string | null; error?: string | null };
type NetworkDiscoveryStatus = {
  enabled: boolean;
  port: number;
  nodeName: string;
  platform: string;
  protocolVersion: number;
};

type VirtualBusRecord = {
  id: string;
  name: string;
  backend: string;
  associationId?: string | null;
};

type VirtualMidiBackendStatus = {
  platform: string;
  available: boolean;
  message: string;
};

type MidiRouteTransform = {
  inputChannel: number | null;
  outputChannel: number | null;
  transpose: number;
  velocityPercent: number;
  ccFrom: number | null;
  ccTo: number | null;
  blockTiming: boolean;
  blockSysex: boolean;
};

type MidiRouteRecord = {
  id: string;
  name: string;
  inputName: string;
  outputName: string;
  enabled: boolean;
  transform: MidiRouteTransform;
};

type MidiRouteRuntimeStatus = {
  route: MidiRouteRecord;
  active: boolean;
  error?: string | null;
};

const tabs: { id: Tab; label: string; icon: typeof Cable }[] = [
  { id: 'midi', label: 'MIDI', icon: Cable },
  { id: 'routing', label: 'ROUTING', icon: Route },
  { id: 'monitor', label: 'MONITOR', icon: Activity },
  { id: 'ndi', label: 'NDI', icon: Radio },
  { id: 'settings', label: 'SETTINGS', icon: Settings }
];

const channels = Array.from({ length: 16 }, (_, index) => index + 1);

function bytesToMessage(bytes: number[]) {
  if (!bytes.length) return 'EMPTY';
  const status = bytes[0];
  const hi = status & 0xf0;
  const channel = (status & 0x0f) + 1;
  if (hi === 0x90 && bytes.length > 2) return `NOTE ON  ch ${channel}  note ${bytes[1]}  vel ${bytes[2]}`;
  if (hi === 0x80 && bytes.length > 2) return `NOTE OFF ch ${channel}  note ${bytes[1]}  vel ${bytes[2]}`;
  if (hi === 0xb0 && bytes.length > 2) return `CC       ch ${channel}  cc ${bytes[1]}  val ${bytes[2]}`;
  if (hi === 0xc0 && bytes.length > 1) return `PROGRAM  ch ${channel}  ${bytes[1]}`;
  if (status === 0xf8) return 'CLOCK';
  if (status === 0xfa) return 'START';
  if (status === 0xfb) return 'CONTINUE';
  if (status === 0xfc) return 'STOP';
  return bytes.map((byte) => byte.toString(16).padStart(2, '0').toUpperCase()).join(' ');
}

function routeTransformSummary(transform: MidiRouteTransform) {
  const parts: string[] = [];

  if (transform.inputChannel) parts.push(`IN CH ${transform.inputChannel}`);
  if (transform.outputChannel) parts.push(`OUT CH ${transform.outputChannel}`);
  if (transform.transpose) parts.push(`${transform.transpose > 0 ? '+' : ''}${transform.transpose} ST`);
  if (transform.velocityPercent !== 100) parts.push(`VEL ${transform.velocityPercent}%`);
  if (transform.ccFrom !== null && transform.ccTo !== null) {
    parts.push(`CC ${transform.ccFrom}→${transform.ccTo}`);
  }
  if (transform.blockTiming) parts.push('NO CLOCK');
  if (transform.blockSysex) parts.push('NO SYSEX');

  return parts.length ? parts : ['PASS THROUGH'];
}

export default function App() {
  const [tab, setTab] = useState<Tab>('midi');
  const [midi, setMidi] = useState<MidiSnapshot>({ inputs: [], outputs: [] });
  const [events, setEvents] = useState<MidiEvent[]>([]);
  const [monitoring, setMonitoring] = useState<number | null>(null);

  const [source, setSource] = useState<number | ''>('');
  const [destination, setDestination] = useState<number | ''>('');
  const [routeName, setRouteName] = useState('');
  const [inputChannel, setInputChannel] = useState<number | ''>('');
  const [outputChannel, setOutputChannel] = useState<number | ''>('');
  const [transpose, setTranspose] = useState(0);
  const [velocityPercent, setVelocityPercent] = useState(100);
  const [ccFrom, setCcFrom] = useState<number | ''>('');
  const [ccTo, setCcTo] = useState<number | ''>('');
  const [blockTiming, setBlockTiming] = useState(false);
  const [blockSysex, setBlockSysex] = useState(false);
  const [savedRoutes, setSavedRoutes] = useState<MidiRouteRuntimeStatus[]>([]);

  const [virtualName, setVirtualName] = useState('LumaLink Bus 1');
  const [virtualBuses, setVirtualBuses] = useState<VirtualBusRecord[]>([]);
  const [virtualBackend, setVirtualBackend] = useState<VirtualMidiBackendStatus>({
    platform: '',
    available: false,
    message: 'Checking virtual MIDI support…'
  });

  const [ndiStatus, setNdiStatus] = useState<NdiStatus>({ available: false });
  const [ndiSources, setNdiSources] = useState<NdiSource[]>([]);
  const [networkStatus, setNetworkStatus] = useState<NetworkDiscoveryStatus | null>(null);
  const [notice, setNotice] = useState('');

  const activeRouteCount = savedRoutes.filter((status) => status.active).length;

  async function refreshMidi() {
    try {
      setMidi(await invoke<MidiSnapshot>('list_midi_devices'));
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function refreshRoutes() {
    try {
      setSavedRoutes(await invoke<MidiRouteRuntimeStatus[]>('list_saved_midi_routes'));
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function refreshVirtualMidi() {
    try {
      const [buses, backend] = await Promise.all([
        invoke<VirtualBusRecord[]>('list_virtual_midi_buses'),
        invoke<VirtualMidiBackendStatus>('virtual_midi_backend_status')
      ]);
      setVirtualBuses(buses);
      setVirtualBackend(backend);
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function refreshNetwork() {
    try {
      setNetworkStatus(await invoke<NetworkDiscoveryStatus>('network_discovery_status'));
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function refreshNdi() {
    try {
      const status = await invoke<NdiStatus>('ndi_runtime_status');
      setNdiStatus(status);
      if (status.available) {
        setNdiSources(await invoke<NdiSource[]>('discover_ndi_sources', { timeoutMs: 1200 }));
      } else {
        setNdiSources([]);
      }
    } catch (error) {
      setNotice(String(error));
    }
  }

  useEffect(() => {
    void refreshMidi();
    void refreshRoutes();
    void refreshVirtualMidi();
    void refreshNdi();
    void refreshNetwork();

    const unlisten = listen<MidiEvent>('midi-event', ({ payload }) => {
      setEvents((current) => [payload, ...current].slice(0, 500));
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  async function toggleMonitor(index: number) {
    try {
      if (monitoring === index) {
        await invoke('stop_midi_monitor');
        setMonitoring(null);
        return;
      }

      await invoke('start_midi_monitor', { inputIndex: index });
      setMonitoring(index);
      setTab('monitor');
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function createVirtualBus() {
    try {
      const result = await invoke<VirtualBusRecord>('create_virtual_midi_bus', { name: virtualName });
      setNotice(`Created virtual MIDI bus: ${result.name}`);
      await Promise.all([refreshMidi(), refreshVirtualMidi()]);
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function removeVirtualBus(id: string, name: string) {
    try {
      await invoke('remove_virtual_midi_bus', { id });
      setNotice(`Removed virtual MIDI bus: ${name}`);
      await Promise.all([refreshMidi(), refreshVirtualMidi(), refreshRoutes()]);
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function saveRoute() {
    if (source === '' || destination === '') return;

    const input = midi.inputs.find((device) => device.index === source);
    const output = midi.outputs.find((device) => device.index === destination);

    if (!input || !output) {
      setNotice('The selected MIDI endpoint disappeared. Refresh devices and try again.');
      return;
    }

    const route: MidiRouteRecord = {
      id: '',
      name: routeName.trim() || `${input.name} → ${output.name}`,
      inputName: input.name,
      outputName: output.name,
      enabled: true,
      transform: {
        inputChannel: inputChannel === '' ? null : inputChannel,
        outputChannel: outputChannel === '' ? null : outputChannel,
        transpose,
        velocityPercent,
        ccFrom: ccFrom === '' ? null : ccFrom,
        ccTo: ccTo === '' ? null : ccTo,
        blockTiming,
        blockSysex
      }
    };

    try {
      const status = await invoke<MidiRouteRuntimeStatus>('save_midi_route', { route });
      setNotice(
        status.error
          ? `Saved ${status.route.name}, but it is offline: ${status.error}`
          : `Saved and started ${status.route.name}`
      );
      setRouteName('');
      await refreshRoutes();
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function toggleSavedRoute(status: MidiRouteRuntimeStatus) {
    try {
      const next = await invoke<MidiRouteRuntimeStatus>('set_midi_route_enabled', {
        id: status.route.id,
        enabled: !status.route.enabled
      });

      setNotice(
        next.error
          ? `${next.route.name} is enabled but offline: ${next.error}`
          : `${next.route.name} ${next.route.enabled ? 'enabled' : 'disabled'}`
      );
      await refreshRoutes();
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function deleteRoute(status: MidiRouteRuntimeStatus) {
    try {
      await invoke('delete_midi_route', { id: status.route.id });
      setNotice(`Removed route: ${status.route.name}`);
      await refreshRoutes();
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function reconnectRoutes() {
    try {
      const statuses = await invoke<MidiRouteRuntimeStatus[]>('reconnect_enabled_midi_routes');
      setSavedRoutes(statuses);

      const failed = statuses.filter((status) => status.route.enabled && !status.active);
      setNotice(
        failed.length
          ? `Reconnected routes. ${failed.length} route${failed.length === 1 ? '' : 's'} still offline.`
          : 'All enabled MIDI routes are connected.'
      );
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function stopRoutes() {
    try {
      await invoke('stop_all_midi_routes');
      setNotice('All runtime MIDI routes stopped. Saved route settings were kept.');
      await refreshRoutes();
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function panic() {
    try {
      await invoke('midi_panic');
      setNotice('Panic sent to all MIDI outputs');
    } catch (error) {
      setNotice(String(error));
    }
  }

  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">L</span>
          <div>
            <strong>LumaLink</strong>
            <small>SYSTEM I/O</small>
          </div>
        </div>

        <div className="status-cluster">
          <span className="status-dot online" /> MIDI ENGINE
          <span className={`status-dot ${ndiStatus.available ? 'online' : ''}`} />
          NDI {ndiStatus.available ? 'READY' : 'RUNTIME OFFLINE'}
          <button className="panic" onClick={panic}>PANIC</button>
        </div>
      </header>

      <nav className="tabs">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            className={tab === id ? 'active' : ''}
            onClick={() => setTab(id)}
          >
            <Icon size={16} />
            {label}
          </button>
        ))}
      </nav>

      {notice && <div className="notice" onClick={() => setNotice('')}>{notice}</div>}

      <section className="content">
        {tab === 'midi' && (
          <>
            <div className="section-head">
              <div>
                <p className="eyebrow">ENDPOINT REGISTRY</p>
                <h1>MIDI Devices</h1>
              </div>
              <button onClick={refreshMidi}><RefreshCw size={15}/> Refresh</button>
            </div>

            <div className="device-grid">
              <Panel title="INPUTS" icon={<Usb size={16}/>}>
                {midi.inputs.length === 0 ? (
                  <Empty text="No MIDI inputs detected" />
                ) : (
                  midi.inputs.map((device) => (
                    <DeviceRow
                      key={device.id}
                      device={device}
                      action={
                        <button
                          className={monitoring === device.index ? 'danger-small' : 'ghost'}
                          onClick={() => toggleMonitor(device.index)}
                        >
                          {monitoring === device.index ? 'STOP' : 'MONITOR'}
                        </button>
                      }
                    />
                  ))
                )}
              </Panel>

              <Panel title="OUTPUTS" icon={<Send size={16}/>}>
                {midi.outputs.length === 0 ? (
                  <Empty text="No MIDI outputs detected" />
                ) : (
                  midi.outputs.map((device) => <DeviceRow key={device.id} device={device} />)
                )}
              </Panel>
            </div>

            <Panel title="VIRTUAL BUSES" icon={<Network size={16}/>}>
              <div className="runtime-line">
                <span className={`big-dot ${virtualBackend.available ? 'online' : ''}`} />
                <div>
                  <strong>
                    {virtualBackend.platform || 'System'} virtual MIDI
                    {virtualBackend.available ? ' ready' : ' unavailable'}
                  </strong>
                  <small>{virtualBackend.message}</small>
                </div>
              </div>

              <div className="inline-form virtual-create">
                <input
                  value={virtualName}
                  onChange={(event) => setVirtualName(event.target.value)}
                  placeholder="Bus name"
                />
                <button
                  className="primary"
                  onClick={createVirtualBus}
                  disabled={!virtualBackend.available || !virtualName.trim()}
                >
                  <Plus size={15}/> Create Bus
                </button>
              </div>

              <div className="virtual-bus-list">
                {virtualBuses.length === 0 ? (
                  <Empty text="No LumaLink virtual buses configured." />
                ) : (
                  virtualBuses.map((bus) => (
                    <div className="device-row" key={bus.id}>
                      <span className="status-dot online"/>
                      <div>
                        <strong>{bus.name}</strong>
                        <small>
                          {bus.backend === 'coremidi'
                            ? 'COREMIDI · APPS ↔ LUMALINK'
                            : 'WINDOWS MIDI SERVICES · COMPATIBILITY LOOPBACK'}
                        </small>
                      </div>
                      <button
                        className="danger-small"
                        onClick={() => removeVirtualBus(bus.id, bus.name)}
                      >
                        REMOVE
                      </button>
                    </div>
                  ))
                )}
              </div>

              <p className="help">
                Saved buses are restored when LumaLink launches. Closing the window leaves LumaLink running in the system tray so active endpoints stay alive.
              </p>
            </Panel>
          </>
        )}

        {tab === 'routing' && (
          <>
            <div className="section-head">
              <div>
                <p className="eyebrow">PATCH BAY + TRANSLATOR</p>
                <h1>MIDI Routing</h1>
              </div>
              <span className={`route-state ${activeRouteCount ? 'live' : ''}`}>
                {activeRouteCount ? `${activeRouteCount} LIVE` : 'IDLE'}
              </span>
            </div>

            <Panel title="NEW SAVED ROUTE" icon={<Route size={16}/>}>
              <div className="route-name-row">
                <label>
                  ROUTE NAME
                  <input
                    value={routeName}
                    onChange={(event) => setRouteName(event.target.value)}
                    placeholder="Optional. Example: Keys → LumaStudio"
                  />
                </label>
              </div>

              <div className="route-builder">
                <label>
                  SOURCE
                  <select
                    value={source}
                    onChange={(event) => setSource(event.target.value === '' ? '' : Number(event.target.value))}
                  >
                    <option value="">Select MIDI input</option>
                    {midi.inputs.map((device) => (
                      <option key={device.id} value={device.index}>{device.name}</option>
                    ))}
                  </select>
                </label>

                <div className="route-arrow">→</div>

                <label>
                  DESTINATION
                  <select
                    value={destination}
                    onChange={(event) => setDestination(event.target.value === '' ? '' : Number(event.target.value))}
                  >
                    <option value="">Select MIDI output</option>
                    {midi.outputs.map((device) => (
                      <option key={device.id} value={device.index}>{device.name}</option>
                    ))}
                  </select>
                </label>
              </div>

              <div className="transform-grid">
                <label>
                  INPUT CHANNEL
                  <select
                    value={inputChannel}
                    onChange={(event) => setInputChannel(event.target.value === '' ? '' : Number(event.target.value))}
                  >
                    <option value="">Any</option>
                    {channels.map((channel) => <option key={channel} value={channel}>{channel}</option>)}
                  </select>
                </label>

                <label>
                  OUTPUT CHANNEL
                  <select
                    value={outputChannel}
                    onChange={(event) => setOutputChannel(event.target.value === '' ? '' : Number(event.target.value))}
                  >
                    <option value="">Same</option>
                    {channels.map((channel) => <option key={channel} value={channel}>{channel}</option>)}
                  </select>
                </label>

                <label>
                  TRANSPOSE
                  <input
                    type="number"
                    min={-48}
                    max={48}
                    value={transpose}
                    onChange={(event) => setTranspose(Number(event.target.value))}
                  />
                </label>

                <label>
                  VELOCITY %
                  <input
                    type="number"
                    min={1}
                    max={200}
                    value={velocityPercent}
                    onChange={(event) => setVelocityPercent(Number(event.target.value))}
                  />
                </label>

                <label>
                  CC FROM
                  <input
                    type="number"
                    min={0}
                    max={127}
                    value={ccFrom}
                    placeholder="Off"
                    onChange={(event) => setCcFrom(event.target.value === '' ? '' : Number(event.target.value))}
                  />
                </label>

                <label>
                  CC TO
                  <input
                    type="number"
                    min={0}
                    max={127}
                    value={ccTo}
                    placeholder="Off"
                    onChange={(event) => setCcTo(event.target.value === '' ? '' : Number(event.target.value))}
                  />
                </label>
              </div>

              <div className="route-options">
                <label className="check-option">
                  <input
                    type="checkbox"
                    checked={blockTiming}
                    onChange={(event) => setBlockTiming(event.target.checked)}
                  />
                  Block MIDI clock / transport
                </label>
                <label className="check-option">
                  <input
                    type="checkbox"
                    checked={blockSysex}
                    onChange={(event) => setBlockSysex(event.target.checked)}
                  />
                  Block SysEx
                </label>
              </div>

              <div className="button-row">
                <button
                  className="primary"
                  onClick={saveRoute}
                  disabled={source === '' || destination === ''}
                >
                  <Plus size={15}/> Save + Enable
                </button>
                <button onClick={reconnectRoutes}><RefreshCw size={15}/> Reconnect Enabled</button>
                <button onClick={stopRoutes}><CircleStop size={15}/> Stop Runtime</button>
              </div>
            </Panel>

            <Panel title={`SAVED ROUTES · ${savedRoutes.length}`} icon={<SquareTerminal size={16}/>}>
              {savedRoutes.length === 0 ? (
                <Empty text="No saved routes yet." />
              ) : (
                <div className="saved-route-list">
                  {savedRoutes.map((status) => (
                    <div className="saved-route" key={status.route.id}>
                      <div className="saved-route-main">
                        <span className={`status-dot ${status.active ? 'online' : ''}`} />
                        <div className="saved-route-copy">
                          <strong>{status.route.name}</strong>
                          <small>{status.route.inputName} → {status.route.outputName}</small>
                          <div className="route-chips">
                            {routeTransformSummary(status.route.transform).map((part) => (
                              <span key={part}>{part}</span>
                            ))}
                          </div>
                          {status.error && <p className="route-error">{status.error}</p>}
                        </div>
                      </div>

                      <div className="saved-route-actions">
                        <span className={`route-badge ${status.active ? 'live' : status.route.enabled ? 'offline' : ''}`}>
                          {status.active ? 'LIVE' : status.route.enabled ? 'OFFLINE' : 'DISABLED'}
                        </span>
                        <button
                          className="ghost"
                          onClick={() => toggleSavedRoute(status)}
                        >
                          {status.route.enabled ? 'DISABLE' : 'ENABLE'}
                        </button>
                        <button
                          className="danger-small"
                          onClick={() => deleteRoute(status)}
                        >
                          REMOVE
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </Panel>

            <Panel title="ROUTER BEHAVIOR" icon={<SquareTerminal size={16}/>}>
              <div className="rule-grid">
                <span>NAME-BOUND PORTS</span>
                <span>PERSISTENT ROUTES</span>
                <span>BYTE TRANSFORMS</span>
                <span>MANUAL RECONNECT</span>
              </div>
              <p className="help">
                Routes are saved against endpoint names instead of volatile port numbers. Enabled routes are restored at launch, stay alive when the window closes, and can be reconnected after a USB device returns.
              </p>
            </Panel>
          </>
        )}

        {tab === 'monitor' && (
          <>
            <div className="section-head">
              <div>
                <p className="eyebrow">LIVE TRAFFIC</p>
                <h1>MIDI Monitor</h1>
              </div>
              <button onClick={() => setEvents([])}>Clear</button>
            </div>

            <div className="monitor-console">
              {events.length === 0 ? (
                <Empty text="Choose MONITOR on a MIDI input or start a saved route." />
              ) : (
                events.map((event, index) => (
                  <div className="monitor-row" key={`${event.timestamp}-${index}`}>
                    <span>{event.timestamp.toString().padStart(10, '0')}</span>
                    <strong>{event.source}</strong>
                    <code>{bytesToMessage(event.bytes)}</code>
                    <small>
                      {event.bytes
                        .map((byte) => byte.toString(16).padStart(2, '0'))
                        .join(' ')
                        .toUpperCase()}
                    </small>
                  </div>
                ))
              )}
            </div>
          </>
        )}

        {tab === 'ndi' && (
          <>
            <div className="section-head">
              <div>
                <p className="eyebrow">NETWORK VIDEO</p>
                <h1>NDI Manager</h1>
              </div>
              <button onClick={refreshNdi}><RefreshCw size={15}/> Discover</button>
            </div>

            <Panel title="RUNTIME" icon={<Radio size={16}/>}>
              <div className="runtime-line">
                <span className={`big-dot ${ndiStatus.available ? 'online' : ''}`} />
                <div>
                  <strong>
                    {ndiStatus.available ? 'NDI runtime loaded' : 'NDI runtime not found'}
                  </strong>
                  <small>
                    {ndiStatus.library || ndiStatus.error || 'Install the free NDI Runtime to enable discovery.'}
                  </small>
                </div>
              </div>
            </Panel>

            <Panel title={`SOURCES · ${ndiSources.length}`} icon={<Network size={16}/>}>
              {ndiSources.length === 0 ? (
                <Empty
                  text={
                    ndiStatus.available
                      ? 'No NDI sources found on this network.'
                      : 'NDI discovery will activate when the runtime is installed.'
                  }
                />
              ) : (
                ndiSources.map((item) => (
                  <div className="ndi-row" key={`${item.name}-${item.url}`}>
                    <span className="status-dot online"/>
                    <div>
                      <strong>{item.name}</strong>
                      <small>{item.url || 'NDI source'}</small>
                    </div>
                    <span className="source-ready">DISCOVERED</span>
                  </div>
                ))
              )}
            </Panel>
          </>
        )}

        {tab === 'settings' && (
          <>
            <div className="section-head">
              <div>
                <p className="eyebrow">SYSTEM</p>
                <h1>Settings</h1>
              </div>
            </div>

            <Panel title="LAN DISCOVERY" icon={<Network size={16}/>}>
              <div className="runtime-line">
                <span className={`big-dot ${networkStatus?.enabled ? 'online' : ''}`} />
                <div>
                  <strong>
                    {networkStatus?.enabled ? `${networkStatus.nodeName} is discoverable` : 'LAN discovery unavailable'}
                  </strong>
                  <small>
                    {networkStatus
                      ? `${networkStatus.platform.toUpperCase()} · UDP ${networkStatus.port} · protocol v${networkStatus.protocolVersion}`
                      : 'Checking LumaLink network discovery…'}
                  </small>
                </div>
                <button onClick={refreshNetwork}><RefreshCw size={14}/> Refresh</button>
              </div>
              <p className="help">
                LumaStudio can use this beacon to find this computer on the same wired or Wi-Fi network, then connect directly to local services such as ProPresenter.
              </p>
            </Panel>

            <Panel title="BUILD PROFILE" icon={<Settings size={16}/>}>
              <div className="settings-list">
                <div>
                  <strong>Background system utility</strong>
                  <small>Closing the window hides LumaLink to the tray instead of destroying virtual MIDI endpoints or active routes. Use Quit from the tray to stop the process.</small>
                </div>
                <div>
                  <strong>Saved routing</strong>
                  <small>Routes bind to endpoint names and restore when LumaLink launches. Reconnect Enabled retries missing USB endpoints without changing the saved patch.</small>
                </div>
                <div>
                  <strong>macOS test signing</strong>
                  <small>Ad-hoc signing is enabled for test DMGs. Production Developer ID notarization stays separate.</small>
                </div>
                <div>
                  <strong>Windows installer</strong>
                  <small>NSIS .exe test build. Authenticode can be layered in later without changing application code.</small>
                </div>
              </div>
            </Panel>
          </>
        )}
      </section>
    </main>
  );
}

function Panel({
  title,
  icon,
  children
}: {
  title: string;
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="panel">
      <header>{icon}<span>{title}</span></header>
      <div className="panel-body">{children}</div>
    </section>
  );
}

function Empty({ text }: { text: string }) {
  return <div className="empty"><Unplug size={18}/>{text}</div>;
}

function DeviceRow({
  device,
  action
}: {
  device: MidiDevice;
  action?: ReactNode;
}) {
  return (
    <div className="device-row">
      <span className="status-dot online"/>
      <div>
        <strong>{device.name}</strong>
        <small>{device.direction.toUpperCase()} · PORT {device.index + 1}</small>
      </div>
      {action}
    </div>
  );
}
