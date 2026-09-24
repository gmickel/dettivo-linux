//! A PipeWire capture stream on the graph's loop: 16 kHz mono s16 requested
//! from PipeWire, PCM and level events on a channel, default following
//! (the default source, or the default sink's monitor for a meeting's
//! system track) reported as `DeviceChanged`, and a pinned device that
//! vanishes reported as `Ended { DeviceLost }` the moment its node leaves
//! the graph.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::stream::{StreamFlags, StreamListener, StreamRc, StreamState};
use spa::param::audio::{AudioFormat, AudioInfoRaw};
use spa::pod::Pod;

use crate::graph::Graph;
use crate::level::Meter;
use crate::{CaptureError, EndReason, Event, SAMPLE_RATE, Target};

#[path = "capture_start.rs"]
mod startup;
use startup::Watch;

/// A running capture.
pub struct Capture {
    events: Option<Receiver<Event>>,
    tx: Sender<Event>,
    stream: StreamRc,
    _listener: StreamListener<CaptureData>,
    loop_: pw::thread_loop::ThreadLoopRc,
    stopped: Arc<AtomicBool>,
    ended_sent: Arc<Mutex<bool>>,
    watcher: Option<std::thread::JoinHandle<()>>,
    watcher_stop: Arc<AtomicBool>,
}

struct CaptureData {
    tx: Sender<Event>,
    meter: Meter,
    stopped: Arc<AtomicBool>,
    /// Set by whoever sends `Ended`, so it is sent exactly once.
    ended_sent: Arc<Mutex<bool>>,
}

impl CaptureData {
    fn publish(&mut self, samples: Vec<i16>) {
        let ended = self.ended_sent.lock().unwrap_or_else(|p| p.into_inner());
        if *ended || samples.is_empty() {
            return;
        }
        for (rms, peak) in self.meter.push(&samples) {
            let _ = self.tx.send(Event::Level { rms, peak });
        }
        let _ = self.tx.send(Event::Pcm(samples));
    }
}

fn format_params() -> Vec<u8> {
    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::S16LE);
    info.set_rate(SAMPLE_RATE);
    info.set_channels(1);
    let object = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .expect("serialize format pod")
    .0
    .into_inner()
}

impl Capture {
    /// Opens a capture on `target` with meter samples every
    /// `level_interval_ms`.
    pub fn open(
        graph: &Graph,
        target: Target,
        level_interval_ms: u64,
    ) -> Result<Self, CaptureError> {
        let (tx, rx) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let guard = graph.loop_.lock();
        let started = startup::start(
            &target,
            || graph.snapshot(),
            || match &target {
                Target::Default => Watch::Default(graph.watch_default()),
                Target::SystemMonitor => Watch::Default(graph.watch_default_sink()),
                Target::Node(name) => Watch::Pinned {
                    name: name.clone(),
                    removed: graph.watch_removed(),
                },
            },
            |node| {
                let mut props = properties! {
                    *pw::keys::MEDIA_TYPE => "Audio",
                    *pw::keys::MEDIA_CATEGORY => "Capture",
                    *pw::keys::MEDIA_ROLE => "Communication",
                    *pw::keys::APP_NAME => "dettivod",
                };
                // A pinned node is the stream's fixed target. The default is left
                // to the session manager: without `target.object` PipeWire connects
                // the stream to the default source (or, with `stream.capture.sink`,
                // to the default sink's monitor) and moves it when the default
                // changes, which is what "follow the default" means; the watcher
                // only reports the change. A node that is a sink is captured
                // through its monitor.
                if let Some(n) = &node {
                    if matches!(target, Target::Node(_)) {
                        props.insert("target.object", n.name.as_str());
                    }
                    if n.is_sink {
                        props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");
                    }
                }
                let stream = StreamRc::new(graph.core.clone(), "dettivo-capture", props)
                    .map_err(|e| CaptureError::Stream(e.to_string()))?;
                let ended_sent = Arc::new(Mutex::new(false));
                let data = CaptureData {
                    tx: tx.clone(),
                    meter: Meter::new(level_interval_ms),
                    stopped: stopped.clone(),
                    ended_sent: ended_sent.clone(),
                };
                let pinned_name = match &target {
                    Target::Node(name) => Some(name.clone()),
                    Target::Default | Target::SystemMonitor => None,
                };
                let listener = stream
                    .add_local_listener_with_user_data(data)
                    .state_changed(move |_stream, data, _old, new| {
                        let mut ended = data.ended_sent.lock().unwrap_or_else(|p| p.into_inner());
                        if *ended {
                            return;
                        }
                        if let StreamState::Error(message) = &new {
                            // A stream error is reported as what it is; a vanished
                            // pinned device shows up as `Unconnected` below.
                            *ended = true;
                            let _ = data.tx.send(Event::Ended {
                                reason: EndReason::Error(message.clone()),
                            });
                        } else if new == StreamState::Unconnected
                            && !data.stopped.load(Ordering::Relaxed)
                        {
                            *ended = true;
                            let reason = match &pinned_name {
                                Some(name) => EndReason::DeviceLost(name.clone()),
                                None => EndReason::NoSource,
                            };
                            let _ = data.tx.send(Event::Ended { reason });
                        }
                    })
                    .process(|stream, data| {
                        let Some(mut buffer) = stream.dequeue_buffer() else {
                            return;
                        };
                        let datas = buffer.datas_mut();
                        let Some(first) = datas.first_mut() else {
                            return;
                        };
                        let size = first.chunk().size() as usize;
                        let offset = first.chunk().offset() as usize;
                        let Some(bytes) = first.data() else {
                            return;
                        };
                        let end = (offset + size).min(bytes.len());
                        let slice = &bytes[offset.min(end)..end];
                        let samples: Vec<i16> = slice
                            .chunks_exact(2)
                            .map(|b| i16::from_le_bytes([b[0], b[1]]))
                            .collect();
                        data.publish(samples);
                    })
                    .register()
                    .map_err(|e| CaptureError::Stream(e.to_string()))?;
                let bytes = format_params();
                let mut params = [Pod::from_bytes(&bytes).expect("format pod")];
                stream
                    .connect(
                        spa::utils::Direction::Input,
                        None,
                        // Not RT_PROCESS: the process callback collects samples and
                        // sends them on a channel, which a real-time thread must not
                        // do; the thread loop's own thread is fine for 16 kHz mono.
                        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
                        &mut params,
                    )
                    .map_err(|e| CaptureError::Stream(e.to_string()))?;
                Ok((stream, listener, ended_sent))
            },
        )?;
        drop(guard);
        let node = started.node;
        let (stream, listener, ended_sent) = started.stream;

        // Default following: report a change of the default source (or
        // sink). A pinned node: report its removal from the graph as the
        // device being lost, since PipeWire keeps a stream on a vanished
        // target alive and feeds it silence.
        let stop_flag = Arc::new(AtomicBool::new(false));
        let watcher = match started.watch {
            Watch::Default(rx_default) => {
                let tx_changed = tx.clone();
                let from = node.as_ref().map(|n| n.name.clone());
                let flag = stop_flag.clone();
                std::thread::Builder::new()
                    .name("dettivo-default-watch".into())
                    .spawn(move || {
                        let mut current = from;
                        while !flag.load(Ordering::Relaxed) {
                            match rx_default.recv_timeout(std::time::Duration::from_millis(100)) {
                                Ok(to) => {
                                    let _ = tx_changed.send(Event::DeviceChanged {
                                        from: current.clone(),
                                        to: to.clone(),
                                    });
                                    current = to;
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    })
                    .ok()
            }
            Watch::Pinned {
                name,
                removed: rx_removed,
            } => {
                let tx_lost = tx.clone();
                let pinned = name.clone();
                let sent = ended_sent.clone();
                let flag = stop_flag.clone();
                std::thread::Builder::new()
                    .name("dettivo-pinned-watch".into())
                    .spawn(move || {
                        while !flag.load(Ordering::Relaxed) {
                            match rx_removed.recv_timeout(std::time::Duration::from_millis(100)) {
                                Ok(gone) if gone == pinned => {
                                    let mut ended = sent.lock().unwrap_or_else(|p| p.into_inner());
                                    if !*ended {
                                        *ended = true;
                                        let _ = tx_lost.send(Event::Ended {
                                            reason: EndReason::DeviceLost(pinned.clone()),
                                        });
                                    }
                                    break;
                                }
                                Ok(_) => {}
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    })
                    .ok()
            }
        };
        let watcher_stop = stop_flag;
        Ok(Self {
            events: Some(rx),
            tx,
            stream,
            _listener: listener,
            loop_: graph.loop_.clone(),
            stopped,
            ended_sent,
            watcher,
            watcher_stop,
        })
    }

    /// Takes the event stream out, so it can move to another thread.
    pub fn take_events(&mut self) -> Receiver<Event> {
        self.events.take().expect("events taken once")
    }

    /// Stops the stream; the event stream ends with `Stopped`.
    pub fn stop(&mut self) {
        if self.stopped.swap(true, Ordering::Relaxed) {
            return;
        }
        self.watcher_stop.store(true, Ordering::Relaxed);
        {
            let _guard = self.loop_.lock();
            let _ = self.stream.disconnect();
        }
        if let Some(handle) = self.watcher.take() {
            let _ = handle.join();
        }
        let mut ended = self.ended_sent.lock().unwrap_or_else(|p| p.into_inner());
        if !*ended {
            *ended = true;
            let _ = self.tx.send(Event::Ended {
                reason: EndReason::Stopped,
            });
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_pcm_is_published_after_the_device_watcher_acknowledges_end() {
        let (tx, rx) = mpsc::channel();
        let ended_sent = Arc::new(Mutex::new(false));
        let mut data = CaptureData {
            tx: tx.clone(),
            meter: Meter::new(50),
            stopped: Arc::new(AtomicBool::new(false)),
            ended_sent: ended_sent.clone(),
        };
        *ended_sent.lock().unwrap() = true;
        tx.send(Event::Ended {
            reason: EndReason::DeviceLost("mic".into()),
        })
        .unwrap();
        data.publish(vec![3000; 320]);
        assert!(matches!(rx.recv().unwrap(), Event::Ended { .. }));
        assert!(
            rx.try_recv().is_err(),
            "PCM after Ended violates the capture boundary"
        );
    }
}
