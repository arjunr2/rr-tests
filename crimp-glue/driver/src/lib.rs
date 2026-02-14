//! CRIMP replay driver for decomposed WebAssembly modules.
//!
//! This crate provides a single entry point [`run_replay`] that provides the
//! core structure for a replay driver. This is intended to be compiled to Wasm and
//! linked with decomposed replay programs to minimize the work needed to support replay
//! in different engines. The logic reads from a trace file, deserializes and
//! dispatches events accordingly.
//!
//! Note that this is not intended to be a complete implementation, but rather a scaffolding
//! program that is further specialized to a set of components (and optionally, even a trace file
//! if the entire trace is available ahead of time).

// Things needed from the host:
// 1. Trace file to read
// 2.

use anyhow::Result;
use env_logger;
use std::fs::File;
use std::io::BufReader;
use wasm_crimp::common_events;
use wasm_crimp::{
    RREvent, RecordSettings, ReplayError, ReplayReader, ReplaySettings, Replayer,
    from_replay_reader,
};

const TRACE_FILEPATH: &str = env!("TRACE_FILEPATH");
const DESERIALIZE_BUFFER_SIZE: Option<&str> = option_env!("DESERIALIZE_BUFFER_SIZE"); // 1 MiB buffer for deserialization

#[cfg(feature = "multi-component")]
compile_error!("Multi-component support is not yet implemented in the Wasm replay driver.");

// ===================================================================================
// Import helpers from host or glue generator
// ===================================================================================
#[link(wasm_import_module = "crimp-host")]
unsafe extern "C" {
    fn log_string(ptr: *const u8, len: usize);
}
/// ===================================================================================
/// [`ReplayBuffer`] implementing a [`Replayer`] (copied mostly implementation from Wasmtime)
/// ===================================================================================

/// Buffer to read replay data
pub struct ReplayBuffer {
    /// Reader to read replay trace from
    reader: Box<dyn ReplayReader>,
    /// Settings in replay configuration
    settings: ReplaySettings,
    /// Settings for record configuration (encoded in the trace)
    trace_settings: RecordSettings,
    /// Intermediate static buffer for deserialization
    deser_buffer: Vec<u8>,
    /// Whether buffer has been completely read
    eof_encountered: bool,
}

impl Iterator for ReplayBuffer {
    type Item = Result<RREvent, ReplayError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.eof_encountered {
            return None;
        }
        let ret = 'event_loop: loop {
            let result = from_replay_reader(&mut *self.reader, &mut self.deser_buffer);
            match result {
                Err(e) => {
                    break 'event_loop Some(Err(ReplayError::FailedRead(e)));
                }
                Ok(event) => {
                    if let RREvent::Eof = &event {
                        self.eof_encountered = true;
                        break 'event_loop None;
                    } else if event.is_diagnostic() {
                        continue 'event_loop;
                    } else {
                        log::debug!("Read replay event => {event}");
                        break 'event_loop Some(Ok(event));
                    }
                }
            }
        };
        ret
    }
}

impl Replayer for ReplayBuffer {
    fn new_replayer(reader: impl ReplayReader + 'static, settings: ReplaySettings) -> Result<Self> {
        let mut buf = ReplayBuffer {
            reader: Box::new(reader),
            deser_buffer: vec![0; settings.deserialize_buffer_size],
            settings,
            // This doesn't matter now; will override after reading header
            trace_settings: RecordSettings::default(),
            eof_encountered: false,
        };
        println!("Deserialize buffer size: {}", buf.deser_buffer.len());

        let signature: common_events::TraceSignatureEvent = buf.next_event_typed()?;
        // NOTE: Trace checksum is not needed to be validated here since this replay
        // format is supposed to be indepedent of the Engine.

        // Update the trace settings
        buf.trace_settings = signature.settings;

        if buf.settings.validate && !buf.trace_settings.add_validation {
            log::warn!(
                "Replay validation will be omitted since the recorded trace has no validation metadata..."
            );
        }

        Ok(buf)
    }

    #[inline]
    fn settings(&self) -> &ReplaySettings {
        &self.settings
    }

    #[inline]
    fn trace_settings(&self) -> &RecordSettings {
        &self.trace_settings
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_replay() {
    env_logger::init();
    log::debug!("Trace file: {}", TRACE_FILEPATH);
    let file = File::open(TRACE_FILEPATH)
        .expect(&format!("Failed to open trace file: {}", TRACE_FILEPATH));
    let reader = BufReader::new(file);

    let mut replayer = ReplayBuffer::new_replayer(
        reader,
        ReplaySettings {
            // For now, we don't support validation in wasm driver
            validate: false,
            deserialize_buffer_size: DESERIALIZE_BUFFER_SIZE
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1024 * 1024), // Default to 1 MiB if not set or invalid
        },
    )
    .unwrap();

    while let Some(event_res) = replayer.next() {
        let event = event_res.unwrap();
    }
}
