//! Bluetooth LE worker. The UI thread never blocks: it sends [`Cmd`]s to a thread that owns a
//! tokio runtime, and gets [`Event`]s back through a channel plus a UI wake-up.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use btleplug::api::{Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use rune::Waker;
use tokio::sync::Semaphore;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::protocol::{self, PACKET_SIZE, Status};

pub enum Cmd {
    Connect,
    Disconnect,
    /// Ask the printer for its model, firmware, battery and paper status again.
    Refresh,
    /// The commands of one print job (see [`protocol::job_with_feed`]).
    Print(Vec<Vec<u8>>),
}

/// What the printer told us about itself. Fields are filled in as the answers arrive.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub battery: Option<u8>,
    /// The raw answer to the paper query, as hex: its format is not documented yet.
    pub paper: Option<String>,
}

pub enum Event {
    Status(String),
    /// The printer's name and signal strength in dBm.
    Connected(String, Option<i16>),
    Disconnected,
    Battery(u8),
    Info(DeviceInfo),
    /// 0.0 to 1.0.
    Progress(f32),
    PrintDone,
    Error(String),
}

/// How long to keep scanning after the first printer shows up, to see whether another is closer.
const SETTLE: Duration = Duration::from_millis(2500);
/// How long to look for a printer at all.
const SCAN_LIMIT: Duration = Duration::from_secs(20);
/// How often to ask a connected, idle printer for its battery and status.
const REFRESH_EVERY: Duration = Duration::from_secs(60);
/// How long to wait for the answer to one query.
const REPLY_WAIT: Duration = Duration::from_millis(1500);

/// Starts the worker thread. `waker` is called after every event so the UI redraws.
pub fn spawn(waker: Waker) -> (UnboundedSender<Cmd>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = unbounded_channel();
    let (ev_tx, ev_rx) = channel();
    if std::env::var_os("TRONIC_FAKE_PRINTER").is_some() {
        // For developing the window without wasting paper: a printer that is always there and
        // "prints" by taking its time. Nothing is sent over Bluetooth.
        std::thread::Builder::new()
            .name("fake-printer".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("tokio runtime");
                rt.block_on(Worker { events: ev_tx, waker, link: None }.run_fake(cmd_rx));
            })
            .expect("spawn fake printer thread");
        return (cmd_tx, ev_rx);
    }
    std::thread::Builder::new()
        .name("ble".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("tokio runtime");
            rt.block_on(Worker { events: ev_tx, waker, link: None }.run(cmd_rx));
        })
        .expect("spawn ble thread");
    (cmd_tx, ev_rx)
}

struct Link {
    peripheral: Peripheral,
    write: Characteristic,
    credits: Arc<Semaphore>,
    /// Answers to queries, from the `FF01` characteristic.
    replies: UnboundedReceiver<Vec<u8>>,
    reader: JoinHandle<()>,
}

struct Worker {
    events: Sender<Event>,
    waker: Waker,
    link: Option<Link>,
}

impl Worker {
    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
        (self.waker)();
    }

    fn say(&self, text: impl Into<String>) {
        self.emit(Event::Status(text.into()));
    }

    async fn run(mut self, mut commands: UnboundedReceiver<Cmd>) {
        let mut refresh = tokio::time::interval(REFRESH_EVERY);
        refresh.tick().await;
        loop {
            tokio::select! {
                cmd = commands.recv() => {
                    let Some(cmd) = cmd else { return };
                    self.handle(cmd).await;
                    refresh.reset();
                }
                _ = refresh.tick() => {
                    if self.link.is_some() {
                        self.refresh_info().await;
                    }
                }
            }
        }
    }

    /// The stand-in printer (see [`spawn`]): connects after a moment, reports a battery and "prints"
    /// for a few seconds, in steps, so the progress effects can be watched.
    async fn run_fake(self, mut commands: UnboundedReceiver<Cmd>) {
        while let Some(cmd) = commands.recv().await {
            match cmd {
                Cmd::Connect => {
                    self.say("Looking for the closest printer…");
                    tokio::time::sleep(Duration::from_millis(900)).await;
                    self.emit(Event::Connected("Fake Printer".into(), Some(-42)));
                    self.emit(Event::Battery(87));
                }
                Cmd::Disconnect => self.emit(Event::Disconnected),
                Cmd::Refresh => self.emit(Event::Battery(87)),
                Cmd::Print(job) => {
                    // Longer jobs take longer, within reason.
                    let steps = (job.len() / 20).clamp(100, 200);
                    for i in 1..=steps {
                        tokio::time::sleep(Duration::from_millis(60)).await;
                        self.emit(Event::Progress(i as f32 / steps as f32));
                    }
                    self.emit(Event::PrintDone);
                }
            }
        }
    }

    async fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Connect => {
                self.drop_link().await;
                match self.connect().await {
                    Ok((link, name, rssi)) => {
                        self.link = Some(link);
                        self.emit(Event::Connected(name, rssi));
                        self.refresh_info().await;
                    }
                    Err(e) => self.emit(Event::Error(e)),
                }
            }
            Cmd::Disconnect => {
                self.drop_link().await;
                self.emit(Event::Disconnected);
            }
            Cmd::Refresh => {
                if self.link.is_some() {
                    self.refresh_info().await;
                }
            }
            Cmd::Print(job) => match self.print(&job).await {
                Ok(()) => self.emit(Event::PrintDone),
                Err(e) => {
                    self.drop_link().await;
                    self.emit(Event::Disconnected);
                    self.emit(Event::Error(e));
                }
            },
        }
    }

    async fn drop_link(&mut self) {
        if let Some(link) = self.link.take() {
            link.reader.abort();
            let _ = link.peripheral.disconnect().await;
        }
    }

    async fn connect(&self) -> Result<(Link, String, Option<i16>), String> {
        self.say("Looking for the closest printer…");
        let manager = Manager::new().await.map_err(|e| format!("Bluetooth is not available: {e}"))?;
        let adapter = manager
            .adapters()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or("no Bluetooth adapter found")?;
        let (peripheral, name, rssi) = find_closest_printer(&adapter).await?;

        self.say(format!("Connecting to {name}…"));
        peripheral.connect().await.map_err(|e| format!("connection failed: {e}"))?;
        peripheral.discover_services().await.map_err(|e| e.to_string())?;
        let chars = peripheral.characteristics();
        let find = |id: u128| chars.iter().find(|c| c.uuid == Uuid::from_u128(id)).cloned();
        let write = find(protocol::WRITE_CHAR).ok_or("this device has no FF02 characteristic; is it the right printer?")?;
        if !write.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
            return Err("FF02 cannot be written without response".into());
        }
        for id in [protocol::NOTIFY_REPLY, protocol::NOTIFY_STATUS] {
            if let Some(c) = find(id) {
                peripheral.subscribe(&c).await.map_err(|e| e.to_string())?;
            }
        }

        let credits = Arc::new(Semaphore::new(0));
        let (reply_tx, replies) = unbounded_channel();
        let mut stream = peripheral.notifications().await.map_err(|e| e.to_string())?;
        let reader = {
            let credits = credits.clone();
            let events = self.events.clone();
            let waker = self.waker.clone();
            tokio::spawn(async move {
                while let Some(n) = stream.next().await {
                    if n.uuid == Uuid::from_u128(protocol::NOTIFY_REPLY) {
                        let _ = reply_tx.send(n.value);
                    } else if n.uuid == Uuid::from_u128(protocol::NOTIFY_STATUS) {
                        match protocol::parse_status(&n.value) {
                            Status::Credits(k) => credits.add_permits(k as usize),
                            Status::Battery(b) => {
                                let _ = events.send(Event::Battery(b));
                                waker();
                            }
                            Status::Other => {}
                        }
                    }
                }
            })
        };
        Ok((Link { peripheral, write, credits, replies, reader }, name, rssi))
    }

    async fn print(&mut self, job: &[Vec<u8>]) -> Result<(), String> {
        let link = self.link.as_ref().ok_or("not connected")?;
        if !link.peripheral.is_connected().await.unwrap_or(false) {
            return Err("the printer disconnected".into());
        }
        let total: usize = job.iter().map(|c| c.len().div_ceil(PACKET_SIZE)).sum();
        let mut sent = 0usize;
        for command in job {
            for packet in command.chunks(PACKET_SIZE) {
                write_packet(link, packet).await?;
                sent += 1;
                if sent % 8 == 0 {
                    self.emit(Event::Progress(sent as f32 / total as f32));
                }
            }
        }
        self.emit(Event::Progress(1.0));
        // Give the printer time to work through its buffer before anything else is sent.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        Ok(())
    }

    /// Ask for model, firmware, battery and paper status, reporting each answer as it comes.
    async fn refresh_info(&mut self) {
        let mut info = DeviceInfo::default();
        let queries: [(&[u8], fn(&mut DeviceInfo, &[u8])); 4] = [
            (&protocol::QUERY_MODEL[..], |i, r| i.model = protocol::parse_text(r)),
            (&protocol::QUERY_FIRMWARE[..], |i, r| i.firmware = protocol::parse_text(r)),
            (&protocol::QUERY_BATTERY[..], |i, r| i.battery = protocol::parse_battery(r)),
            (&protocol::QUERY_PAPER[..], |i, r| i.paper = Some(r.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "))),
        ];
        for (query, store) in queries {
            match self.ask(query).await {
                Ok(Some(reply)) => {
                    store(&mut info, &reply);
                    self.emit(Event::Info(info.clone()));
                }
                Ok(None) => {}
                Err(_) => return,
            }
        }
    }

    /// One query, one answer; `None` if the printer says nothing in time.
    async fn ask(&mut self, query: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let link = self.link.as_mut().ok_or("not connected")?;
        while link.replies.try_recv().is_ok() {}
        for command in protocol::query_job(query) {
            for packet in command.chunks(PACKET_SIZE) {
                write_packet(link, packet).await?;
            }
        }
        Ok(tokio::time::timeout(REPLY_WAIT, link.replies.recv()).await.ok().flatten())
    }
}

/// One packet, once the printer has granted a credit for it.
async fn write_packet(link: &Link, packet: &[u8]) -> Result<(), String> {
    let permit = tokio::time::timeout(Duration::from_secs(8), link.credits.acquire())
        .await
        .map_err(|_| "the printer stopped accepting data (no credits)".to_string())?
        .map_err(|e| e.to_string())?;
    permit.forget();
    link.peripheral.write(&link.write, packet, WriteType::WithoutResponse).await.map_err(|e| format!("write failed: {e}"))
}

/// Scan, and of all the printers seen pick the one with the strongest signal. The scan goes on
/// for a moment after the first sighting so a nearer printer has time to show up.
async fn find_closest_printer(adapter: &Adapter) -> Result<(Peripheral, String, Option<i16>), String> {
    adapter.start_scan(ScanFilter::default()).await.map_err(|e| format!("scan failed: {e}"))?;
    let started = tokio::time::Instant::now();
    let mut first_seen: Option<tokio::time::Instant> = None;
    let mut best: Option<(Peripheral, String, Option<i16>)> = None;
    loop {
        for p in adapter.peripherals().await.map_err(|e| e.to_string())? {
            let Some(props) = p.properties().await.ok().flatten() else { continue };
            let Some(name) = props.local_name.filter(|n| n.starts_with(protocol::NAME)) else { continue };
            first_seen.get_or_insert_with(tokio::time::Instant::now);
            if closer(props.rssi, best.as_ref().and_then(|b| b.2)) || best.is_none() {
                best = Some((p, name, props.rssi));
            }
        }
        let settled = first_seen.is_some_and(|t| t.elapsed() >= SETTLE);
        if settled || started.elapsed() >= SCAN_LIMIT {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let _ = adapter.stop_scan().await;
    best.ok_or_else(|| {
        "Printer not found. Switch it on, close the phone app, turn the phone's Bluetooth off and keep the printer close to the PC.".to_string()
    })
}

/// Is `candidate` a stronger signal than `current`? A known signal beats an unknown one.
fn closer(candidate: Option<i16>, current: Option<i16>) -> bool {
    match (candidate, current) {
        (Some(c), Some(b)) => c > b,
        (Some(_), None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::closer;

    #[test]
    fn the_stronger_signal_is_the_closer_printer() {
        assert!(closer(Some(-50), Some(-70)));
        assert!(!closer(Some(-80), Some(-60)));
        assert!(closer(Some(-90), None), "any reading beats none");
        assert!(!closer(None, Some(-60)));
        assert!(!closer(Some(-60), Some(-60)), "a tie keeps the first one");
    }
}
