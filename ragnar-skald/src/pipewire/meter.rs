// Copyright (C) Jörg Schröder
//! Live microphone level meter.
//!
//! Attaches a lightweight capture stream to a source node and reports
//! the signal peak back to the GUI as [`BackendEvent::MicLevel`]. The
//! stream negotiates F32 audio at the graph rate; the `process`
//! callback runs on PipeWire's realtime data thread and emits a
//! throttled event (max. [`METER_HZ`]) computed from the buffer peak —
//! a few lock-free atomic ops, no allocation, no blocking.
//!
//! The stream is the same mechanism `pavucontrol`'s VU meter uses: it
//! shows up as a tiny recording client while it runs.

use std::io::Cursor;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

use libspa::param::audio::{AudioFormat, AudioInfoRaw};
use libspa::param::format::{MediaSubtype, MediaType};
use libspa::param::format_utils;
use libspa::pod::serialize::PodSerializer;
use libspa::pod::{Object, Pod, Value};
use libspa::utils::SpaTypes;
use pipewire::core::Core;
use pipewire::stream::{Stream, StreamFlags, StreamListener, StreamRef};
use tracing::{debug, warn};

use super::types::BackendEvent;

/// UI update rate for the level meter (Hz) — smooth and cheap.
const METER_HZ: u64 = 20;

/// State shared between the RT `process` callback and the meter
/// owner. All atomics — the callback runs on a different thread.
struct MeterData {
    /// Channel count of the negotiated format (interleave stride).
    channels: AtomicU32,
    /// Peak accumulated across buffers between emits (f32 bits).
    peak: AtomicU32,
    /// Nanos-since-`epoch` of the last emitted level event.
    last_emit: AtomicU64,
    /// Sender for GUI events (unbounded, non-blocking).
    event_tx: mpsc::Sender<BackendEvent>,
    /// Anchor instant so `last_emit` can be compared cheaply.
    epoch: Instant,
}

/// Owns the stream and its listener; dropping disconnects it.
pub struct MicMeter {
    _stream: Stream,
    _listener: StreamListener<()>,
}

impl MicMeter {
    /// Create a capture stream attached to source node `target_id`.
    ///
    /// `None` lets the session manager pick the default source. The
    /// stream negotiates F32 at the native graph rate/channels.
    pub fn new(
        core: &Core,
        target_id: Option<u32>,
        event_tx: mpsc::Sender<BackendEvent>,
    ) -> Option<Self> {
        let props = pipewire::properties::properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Meter",
            "node.name" => "skald.meter",
            "node.description" => "Skald Pegelanzeige",
        };

        let stream = match Stream::new(core, "ragnar-skald-meter", props) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to create meter stream: {e}");
                return None;
            }
        };

        let data = Arc::new(MeterData {
            channels: AtomicU32::new(1),
            peak: AtomicU32::new(0),
            last_emit: AtomicU64::new(0),
            event_tx,
            epoch: Instant::now(),
        });

        let listener = {
            let data_param = Arc::clone(&data);
            let data_process = Arc::clone(&data);
            stream
                .add_local_listener::<()>()
                .param_changed(move |_s, _ud, id, param| {
                    on_param_changed(&data_param, id, param);
                })
                .process(move |s, _ud| on_process(s, &data_process))
                .register()
                .map_err(|e| warn!("Meter listener failed: {e}"))
                .ok()?
        };

        // Offer only F32 so sample bytes are trivially readable.
        let mut audio_info = AudioInfoRaw::new();
        audio_info.set_format(AudioFormat::F32LE);
        let obj = Object {
            type_: SpaTypes::ObjectParamFormat.as_raw(),
            id: libspa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        };
        let bytes = match PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(obj)) {
            Ok((cursor, _)) => cursor.into_inner(),
            Err(e) => {
                warn!("Meter format pod failed: {e}");
                return None;
            }
        };
        let Some(pod) = Pod::from_bytes(&bytes) else {
            warn!("Meter format pod invalid");
            return None;
        };
        let mut params = [pod];

        if let Err(e) = stream.connect(
            libspa::utils::Direction::Input,
            target_id,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut params,
        ) {
            warn!("Meter stream connect failed: {e}");
            return None;
        }

        debug!("Mic meter stream attached to node {target_id:?}");
        Some(Self {
            _stream: stream,
            _listener: listener,
        })
    }
}

/// `param_changed`: learn the negotiated channel count (used as the
/// interleave stride). A NULL param clears the format.
fn on_param_changed(data: &Arc<MeterData>, id: u32, param: Option<&Pod>) {
    let Some(param) = param else { return };
    if id != libspa::param::ParamType::Format.as_raw() {
        return;
    }
    if let Ok((MediaType::Audio, MediaSubtype::Raw)) = format_utils::parse_format(param) {
        let mut info = AudioInfoRaw::new();
        if info.parse(param).is_ok() {
            data.channels
                .store(info.channels().max(1), Ordering::Relaxed);
        }
    }
}

/// RT `process`: fold the buffer peak into `data.peak`, and emit a
/// `MicLevel` event at most [`METER_HZ`] times per second carrying the
/// accumulated peak. Runs on the realtime data thread — a handful of
/// atomic ops per buffer, no allocation, never blocks.
fn on_process(stream: &StreamRef, data: &Arc<MeterData>) {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };
    let datas = buffer.datas_mut();
    let Some(d) = datas.first_mut() else { return };
    let size = d.chunk().size() as usize;
    let Some(raw) = d.data() else { return };

    let stride = data.channels.load(Ordering::Relaxed).max(1) as usize;
    let mut peak = 0.0f32;
    // F32LE interleaved; channel 0 alone is enough for an activity
    // indicator — peak over its frames.
    for frame in raw[..size.min(raw.len())].chunks_exact(4 * stride) {
        if let Ok(arr) = <[u8; 4]>::try_from(&frame[..4]) {
            peak = peak.max(f32::from_le_bytes(arr).abs());
        }
    }
    data.peak
        .fetch_max(peak.min(1.0).to_bits(), Ordering::Relaxed);

    let now = data.epoch.elapsed().as_nanos() as u64;
    let last = data.last_emit.load(Ordering::Relaxed);
    if now.saturating_sub(last) >= 1_000_000_000 / METER_HZ
        && data
            .last_emit
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        let level = f32::from_bits(data.peak.swap(0, Ordering::Relaxed));
        let _ = data.event_tx.send(BackendEvent::MicLevel(level));
    }
}
