use std::io::{self, Read};
use std::sync::mpsc::{SyncSender, sync_channel};

use crate::{OutputStream, Progress};

pub(super) fn collect(
    stdout: Option<impl Read + Send>,
    stderr: Option<impl Read + Send>,
    progress: &mut dyn FnMut(Progress<'_>),
) -> io::Result<(String, String)> {
    let (sender, receiver) = sync_channel(16);
    let mut output = [Vec::new(), Vec::new()];
    let mut emitted = [0, 0];
    std::thread::scope(|scope| {
        let stdout_sender = sender.clone();
        let stdout_reader = scope.spawn(move || pump(stdout, OutputStream::Stdout, stdout_sender));
        let stderr_reader = scope.spawn(move || pump(stderr, OutputStream::Stderr, sender));
        for (stream, bytes) in receiver {
            let index = if stream == OutputStream::Stdout { 0 } else { 1 };
            output[index].extend(bytes);
            emitted[index] += emit(&output[index][emitted[index]..], stream, progress);
        }
        let stdout_result =
            stdout_reader.join().map_err(|_| io::Error::other("stdout reader panicked"))?;
        let stderr_result =
            stderr_reader.join().map_err(|_| io::Error::other("stderr reader panicked"))?;
        stdout_result?;
        stderr_result
    })?;
    for (index, stream) in [OutputStream::Stdout, OutputStream::Stderr].into_iter().enumerate() {
        if emitted[index] < output[index].len() {
            let text = String::from_utf8_lossy(&output[index][emitted[index]..]);
            progress(Progress::Output { stream, text: &text });
        }
    }
    let [stdout, stderr] = output;
    Ok((super::decode(stdout), super::decode(stderr)))
}

fn pump(
    reader: Option<impl Read>,
    stream: OutputStream,
    sender: SyncSender<(OutputStream, Vec<u8>)>,
) -> io::Result<()> {
    let Some(mut reader) = reader else {
        return Ok(());
    };
    loop {
        let mut bytes = vec![0; 8192];
        let count = match reader.read(&mut bytes) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(());
        }
        bytes.truncate(count);
        if sender.send((stream, bytes)).is_err() {
            return Ok(());
        }
    }
}

// Incomplete UTF-8 stays in the collector until the next chunk, rather than becoming U+FFFD.
fn emit(bytes: &[u8], stream: OutputStream, progress: &mut dyn FnMut(Progress<'_>)) -> usize {
    let mut consumed = 0;
    while consumed < bytes.len() {
        match std::str::from_utf8(&bytes[consumed..]) {
            Ok(text) => {
                progress(Progress::Output { stream, text });
                return bytes.len();
            }
            Err(error) => {
                let valid_end = consumed + error.valid_up_to();
                if valid_end > consumed
                    && let Ok(text) = std::str::from_utf8(&bytes[consumed..valid_end])
                {
                    progress(Progress::Output { stream, text });
                }
                consumed = valid_end;
                let Some(invalid) = error.error_len() else {
                    return consumed;
                };
                progress(Progress::Output { stream, text: "\u{fffd}" });
                consumed += invalid;
            }
        }
    }
    consumed
}
