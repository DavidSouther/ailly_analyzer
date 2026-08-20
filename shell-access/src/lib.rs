//! Review a recorded POSIX shell session and report which commands attempted to
//! read from files, write to files, or delete files.
//!
//! Every record this crate emits is a fact about *command text*, never about a
//! disk. Nothing here executes a command, spawns a subprocess, reads the
//! filesystem, or joins a working directory onto an operand. A word the shell
//! would have expanded before any utility saw it is reported as ambiguous
//! rather than resolved into a path or dropped.

use std::path::{Path, PathBuf};

/// Which shell's rules to read a command under. POSIX shell is the only
/// implementation; naming the others here is what lets them arrive later
/// without changing the record shape a caller already consumes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShellLanguage {
    /// Choose an implemented language from the text itself, or refuse.
    AutoDetect,
    Posix,
    /// A language named but not implemented. Classifying under it is an error
    /// rather than an attempt to read it as POSIX.
    Unsupported(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessOperation {
    Read,
    Write,
    /// Kept distinct from `Write` even though a deletion is a write of empty
    /// bytes: this crate's stated goal names deletes, and collapsing them here
    /// would discard something a caller may want.
    Delete,
}

/// Why an operand could not be resolved into a path. Each of these is a word the
/// shell expands before the utility runs, so the text in the command is not the
/// name of a file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmbiguityReason {
    Glob,
    Expansion,
    CommandSubstitution,
    ExpandedHeredoc,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassificationError {
    /// The grammar could not read this command. Reported instead of attributing
    /// the fragments that did parse.
    Parse,
    UnsupportedLanguage {
        language: String,
    },
}

/// One attempted access. `op` and `path` are always present; every other field
/// is omitted unless it carries information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAccess {
    pub op: AccessOperation,
    /// The operand as the command wrote it. Relative stays relative.
    pub path: String,
    /// The directory the classifier was told the command ran in. Copied here as
    /// recorded context and never joined onto `path`.
    pub cwd: Option<PathBuf>,
    /// Set when `path` is a fragment the shell would have expanded rather than a
    /// literal name.
    pub ambiguity: Option<AmbiguityReason>,
    /// True when this access belongs to an inline interpreter invocation, whose
    /// script internals are deliberately not attributed.
    pub scripting: bool,
}

/// Reads commands under one language, optionally carrying the directory a
/// harness recorded for the call. Build one and reuse it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Classifier {
    language: ShellLanguage,
    cwd: Option<PathBuf>,
}

impl AccessOperation {
    /// The word this operation is reported as, shared by every consumer so the
    /// index and the UI cannot drift into two vocabularies.
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

impl AmbiguityReason {
    /// Why the fragment stayed a fragment, phrased for a reader looking at the
    /// row rather than at the grammar.
    pub fn description(self) -> &'static str {
        todo!()
    }
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier {
    /// POSIX shell with no recorded working directory.
    pub const POSIX: Self = Self {
        language: ShellLanguage::Posix,
        cwd: None,
    };

    /// Auto-detects the language of whatever it is given.
    pub fn new() -> Self {
        Self {
            language: ShellLanguage::AutoDetect,
            cwd: None,
        }
    }

    pub fn language(self, language: ShellLanguage) -> Self {
        Self { language, ..self }
    }

    /// Records the directory the command ran in. This is call context, not a
    /// root: it is copied onto each access and never joined onto a path.
    pub fn with_cwd(self, cwd: impl AsRef<Path>) -> Self {
        Self {
            cwd: Some(cwd.as_ref().to_path_buf()),
            ..self
        }
    }

    /// One record per attempted access, in the order the command text names
    /// them. `command` may hold several commands (`&&`, `;`, newlines, pipes);
    /// a command the grammar cannot read is an `Err` at its place in the
    /// stream, so a caller can collect successes and failures independently.
    pub fn classify<'a>(
        &'a self,
        command: &'a str,
    ) -> impl Iterator<Item = Result<FileAccess, ClassificationError>> + 'a {
        let _ = command;
        Vec::new().into_iter()
    }
}
