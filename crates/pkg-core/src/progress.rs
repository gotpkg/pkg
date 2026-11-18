#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Borrowed for the callback only; retain an owned copy if the host needs a log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress<'a> {
    Output { stream: OutputStream, text: &'a str },
    Warning(&'a str),
}

#[derive(Debug)]
pub struct Outcome<T> {
    pub result: T,
    pub warnings: Vec<String>,
}

impl<T> Outcome<T> {
    pub(crate) fn new(result: T) -> Self {
        Self { result, warnings: Vec::new() }
    }

    pub(crate) fn with_warnings(
        result: T,
        warnings: Vec<String>,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Self {
        for warning in &warnings {
            progress(Progress::Warning(warning));
        }
        Self { result, warnings }
    }
}
