//! Assembles the chunks mise writes on each stream into lines.

use pkg_core::OutputStream;

/// The unfinished line of each stream.
#[derive(Default)]
pub(super) struct Lines {
    pending: [String; 2],
}

impl Lines {
    /// The lines `text` completes on `stream`, each with its newline.
    pub(super) fn push(&mut self, stream: OutputStream, text: &str) -> Vec<String> {
        let pending = &mut self.pending[usize::from(stream == OutputStream::Stderr)];
        pending.push_str(text);
        let Some(end) = pending.rfind('\n') else {
            return Vec::new();
        };
        let rest = pending.split_off(end + 1);
        let complete = std::mem::replace(pending, rest);
        complete.split_inclusive('\n').map(str::to_string).collect()
    }

    /// What each stream wrote after its last newline, taken out.
    pub(super) fn take_rest(&mut self) -> Vec<String> {
        self.pending.iter_mut().filter(|rest| !rest.is_empty()).map(std::mem::take).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stream_is_split_into_lines_on_its_own() {
        let mut lines = Lines::default();
        assert!(lines.push(OutputStream::Stderr, "mise brew:wget  ").is_empty());
        assert_eq!(lines.push(OutputStream::Stdout, "out\nmore"), ["out\n"]);
        assert_eq!(
            lines.push(OutputStream::Stderr, "checksum\nmise brew:wget  link\nmise brew"),
            ["mise brew:wget  checksum\n", "mise brew:wget  link\n"]
        );
        assert_eq!(lines.take_rest(), ["more", "mise brew"]);
        assert!(lines.take_rest().is_empty());
    }
}
