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

use crate::protocol::{self, Kind, PACKET_SIZE, Status};

/// A printer found while scanning: enough to show or pick from. The BLE handle travels
/// alongside it in [`discover_printers`]'s result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrinterInfo {
    pub name: String,
    pub kind: Kind,
    pub rssi: Option<i16>,
}

pub enum Cmd {
    /// Connect to the printer the user picked, by the name discovery gave it; `None` keeps the
    /// automatic choice (the strongest Pocket).
    Connect(Option<String>),
    Disconnect,
    /// Ask the printer for its model, firmware, battery and paper status again.
    Refresh,
    /// Scan for printers without changing the current link.
    Scan,
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
    /// The printers found during the last scan, strongest first.
    Discovered(Vec<PrinterInfo>),
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
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                rt.block_on(Worker { events: ev_tx, waker, link: None }.run_fake(cmd_rx));
            })
            .expect("spawn fake printer thread");
        return (cmd_tx, ev_rx);
    }
    std::thread::Builder::new()
        .name("ble".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(Worker { events: ev_tx, waker, link: None }.run(cmd_rx));
        })
        .expect("spawn ble thread");
    (cmd_tx, ev_rx)
}

enum Transport {
    Btleplug { peripheral: Peripheral, write: Characteristic },
    #[cfg(target_os = "linux")]
    RawAtt(raw_att::Conn),
}

struct Link {
    transport: Transport,
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
        let text = text.into();
        eprintln!("[ble] {text}");
        self.emit(Event::Status(text));
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
                Cmd::Connect(_) => {
                    self.say("Looking for the closest printer…");
                    tokio::time::sleep(Duration::from_millis(900)).await;
                    self.emit(Event::Connected("Fake Printer".into(), Some(-42)));
                    self.emit(Event::Battery(87));
                }
                Cmd::Disconnect => self.emit(Event::Disconnected),
                Cmd::Refresh => self.emit(Event::Battery(87)),
                Cmd::Scan => {
                    self.say("Looking for printers…");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    self.emit(Event::Discovered(vec![PrinterInfo {
                        name: "Fake Printer".into(),
                        kind: Kind::Pocket,
                        rssi: Some(-42),
                    }]));
                }
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
            Cmd::Connect(wanted) => {
                self.drop_link().await;
                match self.connect(wanted.as_deref()).await {
                    Ok((link, name, rssi)) => {
                        eprintln!("[ble] connected to {name}");
                        self.link = Some(link);
                        self.emit(Event::Connected(name, rssi));
                        self.refresh_info().await;
                    }
                    Err(e) => {
                        eprintln!("[ble] connect error: {e}");
                        self.emit(Event::Error(e));
                    }
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
            Cmd::Scan => {
                let adapter = match self.adapter().await {
                    Ok(a) => a,
                    Err(e) => {
                        self.emit(Event::Error(e));
                        return;
                    }
                };
                match discover_printers(&adapter).await {
                    Ok(found) => self.emit(Event::Discovered(found.iter().map(|(_, i)| i.clone()).collect())),
                    Err(e) => self.emit(Event::Error(e)),
                }
            }
            Cmd::Print(job) => match self.print(&job).await {
                Ok(()) => self.emit(Event::PrintDone),
                Err(e) => {
                    eprintln!("[ble] print error: {e}");
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
            match link.transport {
                Transport::Btleplug { peripheral, .. } => {
                    let _ = peripheral.disconnect().await;
                }
                #[cfg(target_os = "linux")]
                Transport::RawAtt(_) => {}
            }
        }
    }

    async fn adapter(&self) -> Result<Adapter, String> {
        let manager = Manager::new().await.map_err(|e| format!("Bluetooth is not available: {e}"))?;
        manager
            .adapters()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or("no Bluetooth adapter found".into())
    }

    async fn connect(&self, wanted: Option<&str>) -> Result<(Link, String, Option<i16>), String> {
        match wanted {
            Some(name) => self.say(format!("Looking for {name}…")),
            None => self.say("Looking for the closest printer…"),
        }
        let adapter = self.adapter().await?;
        let found = discover_printers(&adapter).await?;
        let infos: Vec<PrinterInfo> = found.iter().map(|(_, i)| i.clone()).collect();
        self.emit(Event::Discovered(infos.clone()));
        let choice = pick_target(&infos, wanted).ok_or_else(|| {
            "Printer not found. Switch it on, close the phone app, turn the phone's Bluetooth off and keep the printer close to the PC.".to_string()
        })?;
        let (peripheral, info) = &found[choice];
        let (peripheral, name, rssi) = (peripheral.clone(), info.name.clone(), info.rssi);
        let props = peripheral.properties().await.ok().flatten();
        let dev_path = props
            .as_ref()
            .map(|p| format!("/org/bluez/hci0/dev_{}", p.address.to_string().replace(':', "_")));
        let addr_str = props.as_ref().map(|p| p.address.to_string());
        let addr_random = props
            .as_ref()
            .and_then(|p| p.address_type)
            .map(|t| format!("{t:?}").to_lowercase().contains("random"))
            .unwrap_or(false);

        self.say(format!("Connecting to {name}…"));
        let connected = tokio::time::timeout(Duration::from_secs(15), peripheral.connect()).await;
        match connected {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let msg = e.to_string();
                // On BlueZ, dual-mode devices can report BR/EDR "No more profiles to connect to"
                // even when the LE GATT link is (or becomes) usable.
                if msg.contains("No more profiles to connect to") {
                    eprintln!("[ble] connect returned '{msg}'; waiting for BlueZ LE state");
                    // D-Bus Connect may fail due to BR/EDR profile negotiation while LE connects
                    // and resolves. Wait for BlueZ to report a usable state.
                    for i in 0..12 {
                        tokio::time::sleep(Duration::from_millis(700)).await;
                        let (bluez_conn, bluez_resolved) = bluez_device_state(dev_path.as_deref());
                        let plug_conn = peripheral.is_connected().await.unwrap_or(false);
                        if bluez_resolved == Some(true) || bluez_conn == Some(true) || plug_conn {
                            break;
                        }
                        if i == 11 {
                            let _ = peripheral.disconnect().await;
                            return Err(format!("connection failed: {e}"));
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if peripheral.is_connected().await.unwrap_or(false) {
                        eprintln!("[ble] connect reported '{msg}' but device is connected; continuing");
                    } else {
                        let _ = peripheral.disconnect().await;
                        return Err(format!("connection failed: {e}"));
                    }
                }
            }
            Err(_) => {
                // The timed-out connect future is dropped; ask BlueZ to tear down any
                // half-open link so the next attempt does not hit "No more profiles to connect to".
                let _ = peripheral.disconnect().await;
                return Err(format!("connection to {name} timed out"));
            }
        }
        let setup = async {
            // GATT service resolution can lag behind discover_services() on BlueZ; retry a few times
            // before concluding the printer is not exposing the expected characteristics.
            let mut chars = peripheral.characteristics();
            for attempt in 0..8 {
                if attempt == 0 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                if chars.is_empty() {
                    let disc = tokio::time::timeout(Duration::from_secs(10), peripheral.discover_services()).await;
                    if let Ok(Err(e)) = disc {
                        eprintln!("[ble] discover_services error: {e}");
                    }
                    chars = peripheral.characteristics();
                }
                if !chars.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(900)).await;
            }
            let find = |id: u128| chars.iter().find(|c| c.uuid == Uuid::from_u128(id)).cloned();
            let Some(write) = find(protocol::WRITE_CHAR) else {
                if chars.is_empty() {
                    return Err(format!(
                        "connected to {name} but no GATT services were found (BlueZ did not expose characteristics). \
                         Remove the device in your Bluetooth settings (`bluetoothctl remove <address>`), make sure the printer \
                         is in BLE mode, close the phone app, then try Connect again."
                    ));
                }
                return Err("this device has no FF02 characteristic; is it the right printer?".into());
            };
            if !write.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
                return Err("FF02 cannot be written without response".into());
            }

            // Get the notification stream before subscribing so the first credit grants and replies
            // are not missed (they can arrive immediately, especially on BlueZ).
            let mut stream = peripheral.notifications().await.map_err(|e| e.to_string())?;

            let status_char = find(protocol::NOTIFY_STATUS)
                .ok_or("this device has no FF03 status characteristic; flow control will not work")?;
            for c in [find(protocol::NOTIFY_REPLY), Some(status_char)] {
                if let Some(c) = c {
                    tokio::time::timeout(Duration::from_secs(5), peripheral.subscribe(&c))
                        .await
                        .map_err(|_| "subscribing to printer notifications timed out".to_string())?
                        .map_err(|e| e.to_string())?;
                }
            }

            let credits = Arc::new(Semaphore::new(0));
            let (reply_tx, replies) = unbounded_channel();
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
                    let _ = events.send(Event::Disconnected);
                    waker();
                })
            };
            Ok((write, credits, replies, reader))
        }
        .await;

        match setup {
            Ok((write, credits, replies, reader)) => Ok((
                Link {
                    transport: Transport::Btleplug { peripheral, write },
                    credits,
                    replies,
                    reader,
                },
                name,
                rssi,
            )),
            Err(e) => {
                let _ = peripheral.disconnect().await;
                #[cfg(target_os = "linux")]
                {
                    if e.contains("no GATT services were found") {
                        if let Some(addr) = addr_str.as_deref() {
                            eprintln!("[ble] trying raw ATT fallback for {addr}");
                            let credits = Arc::new(Semaphore::new(0));
                            let (reply_tx, replies) = unbounded_channel();
                            let events = self.events.clone();
                            let waker = self.waker.clone();
                            let addr = addr.to_string();
                            let res = tokio::task::spawn_blocking(move || {
                                let c1 = credits.clone();
                                let tx1 = reply_tx.clone();
                                let ev1 = events.clone();
                                let w1 = waker.clone();
                                match raw_att::connect(&addr, addr_random, c1, tx1, ev1, w1) {
                                    Ok((conn, c, reader)) => Ok((conn, c, reader)),
                                    Err(e1) => {
                                        eprintln!("[ble] raw ATT failed: {e1}, trying the other address type");
                                        raw_att::connect(&addr, !addr_random, credits, reply_tx, events, waker)
                                    }
                                }
                            })
                            .await
                            .map_err(|e| format!("raw ATT task failed: {e}"))?;
                            match res {
                                Ok((conn, credits, reader)) => {
                                    return Ok((
                                        Link { transport: Transport::RawAtt(conn), credits, replies, reader },
                                        name,
                                        rssi,
                                    ));
                                }
                                Err(att_e) => {
                                    eprintln!("[ble] raw ATT fallback failed: {att_e}");
                                    return Err(e);
                                }
                            }
                        }
                    }
                }
                Err(e)
            }
        }
    }

    async fn print(&mut self, job: &[Vec<u8>]) -> Result<(), String> {
        let link = self.link.as_ref().ok_or("not connected")?;
        let connected = match &link.transport {
            Transport::Btleplug { peripheral, .. } => peripheral.is_connected().await.unwrap_or(false),
            #[cfg(target_os = "linux")]
            Transport::RawAtt(_) => true,
        };
        if !connected {
            return Err("the printer disconnected".into());
        }
        let total: usize = job.iter().map(|c| c.len().div_ceil(PACKET_SIZE)).sum();
        let mut sent = 0usize;
        for command in job {
            for packet in command.chunks(PACKET_SIZE) {
                write_packet(link, packet, Duration::from_secs(8)).await?;
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
                Ok(None) | Err(_) => {
                    // Queries are best-effort (battery/model/firmware/paper). A timeout or missing
                    // reply must not kill a link that may still print fine.
                }
            }
        }
    }

    /// One query, one answer; `None` if the printer says nothing in time.
    async fn ask(&mut self, query: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let link = self.link.as_mut().ok_or("not connected")?;
        while link.replies.try_recv().is_ok() {}
        for command in protocol::query_job(query) {
            for packet in command.chunks(PACKET_SIZE) {
                write_packet(link, packet, Duration::from_secs(3)).await?;
            }
        }
        Ok(tokio::time::timeout(REPLY_WAIT, link.replies.recv()).await.ok().flatten())
    }
}

/// One packet, once the printer has granted a credit for it.
async fn write_packet(link: &Link, packet: &[u8], credit_wait: Duration) -> Result<(), String> {
    let permit = tokio::time::timeout(credit_wait, link.credits.acquire())
        .await
        .map_err(|_| "the printer stopped accepting data (no credits)".to_string())?
        .map_err(|e| e.to_string())?;
    permit.forget();
    match &link.transport {
        Transport::Btleplug { peripheral, write } => peripheral
            .write(write, packet, WriteType::WithoutResponse)
            .await
            .map_err(|e| format!("write failed: {e}")),
        #[cfg(target_os = "linux")]
        Transport::RawAtt(conn) => conn.write_no_rsp(packet),
    }
}

/// Read BlueZ `Connected` and `ServicesResolved` for a device object path (Linux workaround).
/// Returns `(connected, services_resolved)` as `None` when unavailable.
fn bluez_device_state(dev_path: Option<&str>) -> (Option<bool>, Option<bool>) {
    let Some(path) = dev_path else {
        return (None, None);
    };
    let read = |prop: &str| -> Option<bool> {
        let out = std::process::Command::new("gdbus")
            .args([
                "call",
                "--system",
                "--dest",
                "org.bluez",
                "--object-path",
                path,
                "--method",
                "org.freedesktop.DBus.Properties.Get",
                "org.bluez.Device1",
                prop,
            ])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        if s.contains("true") {
            Some(true)
        } else if s.contains("false") {
            Some(false)
        } else {
            None
        }
    };
    (read("Connected"), read("ServicesResolved"))
}

#[cfg(target_os = "linux")]
mod raw_att {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::sync::mpsc::Sender as StdSender;
    use std::time::Duration;

    use rune::Waker;
    use tokio::sync::Semaphore;
    use tokio::sync::mpsc::UnboundedSender;
    use tokio::task::JoinHandle;

    use super::Event;
    use crate::protocol::{self, Status};

    // ATT opcodes
    const OP_ERROR: u8 = 0x01;
    const OP_MTU_REQ: u8 = 0x02;
    const OP_MTU_RSP: u8 = 0x03;
    const OP_FIND_INFO_REQ: u8 = 0x04;
    const OP_FIND_INFO_RSP: u8 = 0x05;
    const OP_READ_BY_TYPE_REQ: u8 = 0x08;
    const OP_READ_BY_TYPE_RSP: u8 = 0x09;
    const OP_READ_BY_GROUP_TYPE_REQ: u8 = 0x10;
    const OP_READ_BY_GROUP_TYPE_RSP: u8 = 0x11;
    const OP_WRITE_REQ: u8 = 0x12;
    const OP_WRITE_RSP: u8 = 0x13;
    const OP_WRITE_CMD: u8 = 0x52;
    const OP_HANDLE_VALUE_NTF: u8 = 0x1b;

    pub struct Conn {
        fd: OwnedFd,
        pub write_handle: u16,
    }

    impl Conn {
        pub fn write_no_rsp(&self, packet: &[u8]) -> Result<(), String> {
            let mut pdu = Vec::with_capacity(3 + packet.len());
            pdu.push(OP_WRITE_CMD);
            pdu.extend_from_slice(&self.write_handle.to_le_bytes());
            pdu.extend_from_slice(packet);
            send_pdu(&self.fd, &pdu)
        }
    }

    fn send_pdu(fd: &OwnedFd, pdu: &[u8]) -> Result<(), String> {
        let n = unsafe { libc::write(fd.as_raw_fd(), pdu.as_ptr() as *const _, pdu.len()) };
        if n < 0 {
            Err(format!("ATT write failed: {}", io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    fn recv_pdu(fd: &OwnedFd, timeout: Duration) -> Result<Vec<u8>, String> {
        let tv = libc::timeval {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_usec: timeout.subsec_micros() as libc::suseconds_t,
        };
        unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &tv as *const _ as *const _,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
        }
        let mut buf = [0u8; 512];
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
        if n < 0 {
            Err(format!("ATT read failed: {}", io::Error::last_os_error()))
        } else {
            Ok(buf[..n as usize].to_vec())
        }
    }

    fn req_rsp(fd: &OwnedFd, req: &[u8], expect: u8) -> Result<Vec<u8>, String> {
        send_pdu(fd, req)?;
        for _ in 0..8 {
            let pdu = recv_pdu(fd, Duration::from_secs(4))?;
            if pdu.is_empty() {
                continue;
            }
            if pdu[0] == expect {
                return Ok(pdu);
            }
            if pdu[0] == OP_ERROR {
                return Err(format!("ATT error {:02x?}", pdu));
            }
            // skip notifications/other during setup
            if pdu[0] == OP_HANDLE_VALUE_NTF {
                continue;
            }
        }
        Err("ATT: no expected response".into())
    }

    fn parse_addr(addr: &str) -> Option<[u8; 6]> {
        let parts: Vec<&str> = addr.split(':').collect();
        if parts.len() != 6 {
            return None;
        }
        let mut out = [0u8; 6];
        for (i, p) in parts.iter().enumerate() {
            out[5 - i] = u8::from_str_radix(p, 16).ok()?;
        }
        Some(out)
    }

    pub fn connect(
        addr: &str,
        addr_type_random: bool,
        credits: std::sync::Arc<Semaphore>,
        reply_tx: UnboundedSender<Vec<u8>>,
        events: StdSender<Event>,
        waker: Waker,
    ) -> Result<(Conn, std::sync::Arc<Semaphore>, JoinHandle<()>), String> {
        let bd = parse_addr(addr).ok_or("invalid bluetooth address")?;
        let fd = unsafe { libc::socket(31 /*AF_BLUETOOTH*/, libc::SOCK_SEQPACKET, 0 /*BTPROTO_L2CAP*/) };
        if fd < 0 {
            return Err(format!("L2CAP socket failed: {}", io::Error::last_os_error()));
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct SockaddrL2 {
            l2_family: libc::sa_family_t,
            l2_psm: u16,
            l2_bdaddr: [u8; 6],
            l2_cid: u16,
            l2_bdaddr_type: u8,
        }
        const SOL_BLUETOOTH: libc::c_int = 274;
        const BT_SECURITY: libc::c_int = 4;
        const BT_SECURITY_LOW: libc::c_int = 1;
        let att_cid: u16 = 4u16.to_le();

        // Bind source (BDADDR_ANY, any adapter)
        let src = SockaddrL2 {
            l2_family: 31,
            l2_psm: 0,
            l2_bdaddr: [0; 6],
            l2_cid: att_cid,
            l2_bdaddr_type: 0, // BDADDR_BREDR/any for source; public adapter
        };
        let rc = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                &src as *const _ as *const libc::sockaddr,
                std::mem::size_of::<SockaddrL2>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            return Err(format!("L2CAP bind failed: {}", io::Error::last_os_error()));
        }
        // Security level (required on many kernels/BlueZ setups)
        let sec: libc::c_int = BT_SECURITY_LOW;
        unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                SOL_BLUETOOTH,
                BT_SECURITY,
                &sec as *const _ as *const _,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }

        let dst = SockaddrL2 {
            l2_family: 31,
            l2_psm: 0,
            l2_bdaddr: bd,
            l2_cid: att_cid,
            l2_bdaddr_type: if addr_type_random { 2 } else { 1 }, // BDADDR_LE_PUBLIC=1, RANDOM=2
        };
        eprintln!("[ble] raw ATT: connecting L2CAP to {addr} (random={addr_type_random})");
        let rc = unsafe {
            libc::connect(
                fd.as_raw_fd(),
                &dst as *const _ as *const libc::sockaddr,
                std::mem::size_of::<SockaddrL2>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            return Err(format!("L2CAP connect failed: {}", io::Error::last_os_error()));
        }
        eprintln!("[ble] raw ATT: L2CAP connected to {addr}");

        // MTU exchange (best-effort)
        let _ = req_rsp(&fd, &[OP_MTU_REQ, 0xE7, 0x00], OP_MTU_RSP);

        // Discover primary services (type 0x2800) and find 0xFF00
        let svc = req_rsp(
            &fd,
            &[
                OP_READ_BY_GROUP_TYPE_REQ,
                0x01,
                0x00,
                0xFF,
                0xFF,
                0x00,
                0x28, // Primary Service UUID type
            ],
            OP_READ_BY_GROUP_TYPE_RSP,
        )?;
        let (svc_start, svc_end) = parse_ff00_service(&svc).ok_or("FF00 service not found (raw ATT)")?;
        eprintln!("[ble] raw ATT: FF00 service {svc_start:#06x}-{svc_end:#06x}");

        // Discover characteristics 0x2803 in service range
        let mut chars = Vec::new();
        let mut start = svc_start;
        while start <= svc_end {
            let req = [
                OP_READ_BY_TYPE_REQ,
                (start & 0xFF) as u8,
                (start >> 8) as u8,
                (svc_end & 0xFF) as u8,
                (svc_end >> 8) as u8,
                0x03,
                0x28, // Characteristic
            ];
            match req_rsp(&fd, &req, OP_READ_BY_TYPE_RSP) {
                Ok(rsp) => {
                    let found = parse_chars(&rsp, &mut chars);
                    if found == 0 {
                        break;
                    }
                    start = chars.last().map(|c: &(u8, u16, u16)| c.1 + 1).unwrap_or(start + 1);
                }
                Err(e) if e.contains("ATT error") => break,
                Err(e) => return Err(e),
            }
        }
        let mut handle_ff01 = None;
        let mut handle_ff02 = None;
        let mut handle_ff03 = None;
        let mut decl_ff01 = None;
        let mut decl_ff03 = None;
        for (props, val_handle, uuid16) in &chars {
            match *uuid16 {
                0xFF01 => {
                    handle_ff01 = Some(*val_handle);
                    decl_ff01 = Some(*val_handle - 1);
                    let _ = props;
                }
                0xFF02 => handle_ff02 = Some(*val_handle),
                0xFF03 => {
                    handle_ff03 = Some(*val_handle);
                    decl_ff03 = Some(*val_handle - 1);
                    let _ = props;
                }
                _ => {}
            }
        }
        let write_handle = handle_ff02.ok_or("FF02 characteristic not found (raw ATT)")?;
        let _ = handle_ff01.ok_or("FF01 characteristic not found (raw ATT)")?;
        let _ = handle_ff03.ok_or("FF03 characteristic not found (raw ATT)")?;
        eprintln!("[ble] raw ATT: FF01={handle_ff01:?} FF02={write_handle:#06x} FF03={handle_ff03:?}");

        // Enable notifications on FF01 and FF03 by writing CCCD (0x2902) = 0x0001
        for decl in [decl_ff01, decl_ff03].into_iter().flatten() {
            // find CCCD between decl+1 and decl+4 roughly
            let req = [
                OP_FIND_INFO_REQ,
                ((decl + 2) & 0xFF) as u8,
                ((decl + 2) >> 8) as u8,
                ((decl + 5) & 0xFF) as u8,
                ((decl + 5) >> 8) as u8,
            ];
            if let Ok(rsp) = req_rsp(&fd, &req, OP_FIND_INFO_RSP) {
                if let Some(cccd) = parse_cccd(&rsp) {
                    let _ = req_rsp(
                        &fd,
                        &[OP_WRITE_REQ, (cccd & 0xFF) as u8, (cccd >> 8) as u8, 0x01, 0x00],
                        OP_WRITE_RSP,
                    );
                    eprintln!("[ble] raw ATT: enabled notifications, cccd={cccd:#06x}");
                }
            }
        }

        // Duplicate fd for reader
        let read_fd = fd.try_clone().map_err(|e| format!("fd clone: {e}"))?;
        let credits_ret = credits.clone();
        let reader = tokio::task::spawn_blocking(move || {
            notification_loop(read_fd, handle_ff01, handle_ff03, credits, reply_tx, events, waker)
        });

        Ok((Conn { fd, write_handle }, credits_ret, reader))
    }

    fn parse_ff00_service(rsp: &[u8]) -> Option<(u16, u16)> {
        // Read By Group Type Response: opcode, length, data(length)*N
        if rsp.len() < 4 {
            return None;
        }
        let len = rsp[1] as usize;
        if len < 6 {
            return None;
        }
        let mut i = 2;
        while i + len <= rsp.len() {
            let start = u16::from_le_bytes([rsp[i], rsp[i + 1]]);
            let end = u16::from_le_bytes([rsp[i + 2], rsp[i + 3]]);
            // uuid16 present when len == 6
            if len == 6 {
                let uuid = u16::from_le_bytes([rsp[i + 4], rsp[i + 5]]);
                if uuid == 0xFF00 {
                    return Some((start, end));
                }
            }
            i += len;
        }
        None
    }

    fn parse_chars(rsp: &[u8], out: &mut Vec<(u8, u16, u16)>) -> usize {
        if rsp.len() < 4 {
            return 0;
        }
        let len = rsp[1] as usize;
        let mut i = 2;
        let mut n = 0;
        while i + len <= rsp.len() && len >= 5 {
            let props = rsp[i + 2];
            let val = u16::from_le_bytes([rsp[i + 3], rsp[i + 4]]);
            let uuid16 = if len == 7 {
                u16::from_le_bytes([rsp[i + 5], rsp[i + 6]])
            } else {
                0
            };
            out.push((props, val, uuid16));
            i += len;
            n += 1;
        }
        n
    }

    fn parse_cccd(rsp: &[u8]) -> Option<u16> {
        if rsp.len() < 4 || rsp[1] != 1 {
            return None;
        } // format 1 = 16-bit uuids, len 4
        let mut i = 2;
        while i + 4 <= rsp.len() {
            let h = u16::from_le_bytes([rsp[i], rsp[i + 1]]);
            let u = u16::from_le_bytes([rsp[i + 2], rsp[i + 3]]);
            if u == 0x2902 {
                return Some(h);
            }
            i += 4;
        }
        None
    }

    fn notification_loop(
        fd: OwnedFd,
        handle_ff01: Option<u16>,
        handle_ff03: Option<u16>,
        credits: std::sync::Arc<Semaphore>,
        reply_tx: UnboundedSender<Vec<u8>>,
        events: StdSender<Event>,
        waker: Waker,
    ) {
        loop {
            let pdu = match recv_pdu(&fd, Duration::from_secs(30)) {
                Ok(p) => p,
                Err(e) => {
                    if e.contains("Resource temporarily unavailable")
                        || e.contains("timed out")
                        || e.contains("WouldBlock")
                        || e.contains("Try again")
                    {
                        continue;
                    }
                    eprintln!("[ble] raw ATT read error: {e}");
                    break;
                }
            };
            if pdu.is_empty() {
                break;
            }
            if pdu.len() < 3 || pdu[0] != OP_HANDLE_VALUE_NTF {
                continue;
            }
            let handle = u16::from_le_bytes([pdu[1], pdu[2]]);
            let value = &pdu[3..];
            if Some(handle) == handle_ff01 {
                let _ = reply_tx.send(value.to_vec());
            } else if Some(handle) == handle_ff03 {
                match protocol::parse_status(value) {
                    Status::Credits(k) => credits.add_permits(k as usize),
                    Status::Battery(b) => {
                        let _ = events.send(Event::Battery(b));
                        waker();
                    }
                    Status::Other => {}
                }
            }
        }
        let _ = events.send(Event::Disconnected);
        waker();
    }
}

/// Scan for every Tronic-like printer seen during the settle window, deduplicated by id and
/// sorted by RSSI (strongest first, unknown last). A Label printer is included in the list even
/// though it is never auto-connected.
pub async fn discover_printers(adapter: &Adapter) -> Result<Vec<(Peripheral, PrinterInfo)>, String> {
    adapter.start_scan(ScanFilter::default()).await.map_err(|e| format!("scan failed: {e}"))?;
    let started = tokio::time::Instant::now();
    let mut first_seen: Option<tokio::time::Instant> = None;
    // All sightings; dedup happens afterwards, keeping the strongest RSSI per id.
    let mut sightings: Vec<(Peripheral, String, Option<i16>)> = Vec::new();
    loop {
        for p in adapter.peripherals().await.map_err(|e| e.to_string())? {
            let Some(props) = p.properties().await.ok().flatten() else { continue };
            let Some(name) = props.local_name else { continue };
            if protocol::kind_of(&name).is_none() { continue; }
            first_seen.get_or_insert_with(tokio::time::Instant::now);
            sightings.push((p, name, props.rssi));
        }
        let settled = first_seen.is_some_and(|t| t.elapsed() >= SETTLE);
        if settled || started.elapsed() >= SCAN_LIMIT {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let _ = adapter.stop_scan().await;

    // Deduplicate by id, keeping the strongest signal seen for each device.
    let mut by_id: std::collections::BTreeMap<String, (Peripheral, String, Option<i16>)> =
        std::collections::BTreeMap::new();
    for (p, name, rssi) in sightings {
        let id = p.id().to_string();
        match by_id.get(&id) {
            Some((_, _, cur)) if closer(rssi, *cur) => {
                by_id.insert(id, (p, name, rssi));
            }
            None => {
                by_id.insert(id, (p, name, rssi));
            }
            _ => {}
        }
    }

    let mut found: Vec<(Peripheral, PrinterInfo)> = by_id
        .into_values()
        .map(|(peripheral, name, rssi)| {
            let kind = protocol::kind_of(&name).unwrap();
            (peripheral, PrinterInfo { name, kind, rssi })
        })
        .collect();
    found.sort_by(|a, b| cmp_rssi(a.1.rssi, b.1.rssi));
    Ok(found)
}

/// Index of the strongest Pocket printer in a discovered list, if any. Label printers are never
/// chosen here.
pub fn best_pocket_index(found: &[PrinterInfo]) -> Option<usize> {
    found
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind == Kind::Pocket)
        .max_by(|(_, a), (_, b)| cmp_rssi(a.rssi, b.rssi))
        .map(|(i, _)| i)
}

/// The printer to connect to: the one the user picked by name, or the strongest Pocket.
/// `None` when the picked name is not in the scan: the caller says "not found" and never falls
/// back to another printer.
pub fn pick_target(found: &[PrinterInfo], wanted: Option<&str>) -> Option<usize> {
    match wanted {
        Some(name) => found.iter().position(|p| p.name == name),
        None => best_pocket_index(found),
    }
}

/// Ordering for sorting by RSSI, strongest first, unknown last. Returns the ordering of `a`
/// relative to `b` such that a sort_by with this comparator puts the stronger signal first.
fn cmp_rssi(a: Option<i16>, b: Option<i16>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Sort a slice of printer infos in place, strongest RSSI first, unknown last.
fn sort_printers(items: &mut [PrinterInfo]) {
    items.sort_by(|a, b| cmp_rssi(a.rssi, b.rssi));
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
    use super::{Kind, PrinterInfo, best_pocket_index, closer, pick_target, sort_printers};
    use crate::protocol;

    #[test]
    fn the_stronger_signal_is_the_closer_printer() {
        assert!(closer(Some(-50), Some(-70)));
        assert!(!closer(Some(-80), Some(-60)));
        assert!(closer(Some(-90), None), "any reading beats none");
        assert!(!closer(None, Some(-60)));
        assert!(!closer(Some(-60), Some(-60)), "a tie keeps the first one");
    }

    #[test]
    fn discovered_printers_are_sorted_strongest_first() {
        let mut items = vec![
            PrinterInfo { name: "Mini Pocket Printer_BLE".into(), kind: Kind::Pocket, rssi: Some(-80) },
            PrinterInfo { name: "ML Printer_BLE".into(), kind: Kind::Label, rssi: Some(-50) },
            PrinterInfo { name: "Mini Pocket Printer_BLE".into(), kind: Kind::Pocket, rssi: None },
        ];
        sort_printers(&mut items);
        assert_eq!(items[0].name, "ML Printer_BLE", "strongest signal first");
        assert_eq!(items[0].rssi, Some(-50));
        assert_eq!(items[1].rssi, Some(-80));
        assert_eq!(items[2].rssi, None, "unknown RSSI last");
    }

    #[test]
    fn a_label_printer_nearest_still_picks_the_pocket_one_for_connection() {
        let mut items = vec![
            PrinterInfo { name: "ML Printer_BLE".into(), kind: Kind::Label, rssi: Some(-40) },
            PrinterInfo { name: "Mini Pocket Printer_BLE".into(), kind: Kind::Pocket, rssi: Some(-70) },
        ];
        sort_printers(&mut items);
        let idx = best_pocket_index(&items).expect("a pocket printer exists");
        assert_eq!(items[idx].name, "Mini Pocket Printer_BLE", "auto-connect stays on Pocket even when a Label is closer");
        assert_eq!(protocol::kind_of("ML Printer_BLE"), Some(Kind::Label));
        assert_eq!(protocol::kind_of("Mini Pocket Printer_BLE"), Some(Kind::Pocket));
    }

    #[test]
    fn the_chosen_target_is_passed_through_and_a_missing_one_is_not_fallen_back_from() {
        let items = vec![
            PrinterInfo { name: "ML Printer_BLE".into(), kind: Kind::Label, rssi: Some(-40) },
            PrinterInfo { name: "Mini Pocket Printer_BLE".into(), kind: Kind::Pocket, rssi: Some(-70) },
        ];
        assert_eq!(pick_target(&items, Some("ML Printer_BLE")), Some(0), "the picked printer wins, Label or not");
        assert_eq!(pick_target(&items, Some("Mini Pocket Printer_BLE")), Some(1));
        assert_eq!(pick_target(&items, Some("Unknown")), None, "a pick that is not in the scan is a miss, never another printer");
        assert_eq!(pick_target(&items, None), Some(1), "no pick: the strongest Pocket");
    }
}
