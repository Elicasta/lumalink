import { useEffect, useMemo, useState } from 'react';
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

const tabs: { id: Tab; label: string; icon: typeof Cable }[] = [
  { id: 'midi', label: 'MIDI', icon: Cable },
  { id: 'routing', label: 'ROUTING', icon: Route },
  { id: 'monitor', label: 'MONITOR', icon: Activity },
  { id: 'ndi', label: 'NDI', icon: Radio },
  { id: 'settings', label: 'SETTINGS', icon: Settings }
];

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
  return bytes.map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join(' ');
}

export default function App() {
  const [tab, setTab] = useState<Tab>('midi');
  const [midi, setMidi] = useState<MidiSnapshot>({ inputs: [], outputs: [] });
  const [events, setEvents] = useState<MidiEvent[]>([]);
  const [monitoring, setMonitoring] = useState<number | null>(null);
  const [source, setSource] = useState<number | ''>('');
  const [destination, setDestination] = useState<number | ''>('');
  const [routeActive, setRouteActive] = useState(false);
  const [virtualName, setVirtualName] = useState('LumaLink Bus 1');
  const [ndiStatus, setNdiStatus] = useState<NdiStatus>({ available: false });
  const [ndiSources, setNdiSources] = useState<NdiSource[]>([]);
  const [notice, setNotice] = useState('');

  async function refreshMidi() {
    try {
      const snapshot = await invoke<MidiSnapshot>('list_midi_devices');
      setMidi(snapshot);
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
    void refreshNdi();

    const unlisten = listen<MidiEvent>('midi-event', ({ payload }) => {
      setEvents((current) => [payload, ...current].slice(0, 500));
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const inputByIndex = useMemo(
    () => new Map(midi.inputs.map((device) => [device.index, device])),
    [midi.inputs]
  );
  const outputByIndex = useMemo(
    () => new Map(midi.outputs.map((device) => [device.index, device])),
    [midi.outputs]
  );

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
      const result = await invoke<string>('create_virtual_midi_bus', { name: virtualName });
      setNotice(result);
      await refreshMidi();
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function startRoute() {
    if (source === '' || destination === '') return;
    try {
      await invoke('start_midi_route', { inputIndex: source, outputIndex: destination });
      setRouteActive(true);
      setNotice(
        `Routing ${inputByIndex.get(source)?.name ?? source} → ${outputByIndex.get(destination)?.name ?? destination}`
      );
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function stopRoutes() {
    try {
      await invoke('stop_all_midi_routes');
      setRouteActive(false);
      setNotice('MIDI routes stopped');
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

            <Panel title="VIRTUAL BUS" icon={<Network size={16}/>}>
              <div className="inline-form">
                <input
                  value={virtualName}
                  onChange={(event) => setVirtualName(event.target.value)}
                />
                <button className="primary" onClick={createVirtualBus}>
                  <Plus size={15}/> Create Bus
                </button>
              </div>
              <p className="help">
                macOS creates real CoreMIDI virtual input/output endpoints. Windows physical MIDI is active now; native Windows MIDI Services virtual devices are the next backend module.
              </p>
            </Panel>
          </>
        )}

        {tab === 'routing' && (
          <>
            <div className="section-head">
              <div>
                <p className="eyebrow">PATCH BAY</p>
                <h1>MIDI Routing</h1>
              </div>
              <span className={`route-state ${routeActive ? 'live' : ''}`}>
                {routeActive ? 'ROUTING LIVE' : 'IDLE'}
              </span>
            </div>

            <Panel title="NEW ROUTE" icon={<Route size={16}/>}>
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

              <div className="button-row">
                <button
                  className="primary"
                  onClick={startRoute}
                  disabled={source === '' || destination === ''}
                >
                  <Play size={15}/> Start Route
                </button>
                <button onClick={stopRoutes}><CircleStop size={15}/> Stop All</button>
              </div>
            </Panel>

            <Panel title="ROUTER RULES" icon={<SquareTerminal size={16}/>}>
              <div className="rule-grid">
                <span>ONE → ONE</span>
                <span>ONE → MANY</span>
                <span>MANY → ONE</span>
                <span>HOT-PLUG READY</span>
              </div>
              <p className="help">
                Multiple routes can run simultaneously. Mapping and translation stay above this transport layer so routing and message transforms do not get tangled together.
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
                <Empty text="Choose MONITOR on a MIDI input and move a control." />
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

            <Panel title="BUILD PROFILE" icon={<Settings size={16}/>}>
              <div className="settings-list">
                <div>
                  <strong>Background routing service</strong>
                  <small>Reserved as the next service split so UI closure will not own long-running routes.</small>
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
