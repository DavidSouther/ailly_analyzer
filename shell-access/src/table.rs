//! Which of a utility's operands name files, and which of them are something
//! else entirely.
//!
//! A parser inventories words; it does not know that `sed`'s first operand is a
//! script and `tee`'s is a destination. That knowledge is a table, and no
//! published crate holds one in the shape this needs, so this one is ours to
//! write and grow.
//!
//! It is **additive**, in the same spirit as the client's tool `CATEGORY_TABLE`:
//! a utility missing from here contributes no file rows at all. That is a
//! visible under-report, which is the failure this feature can afford. The
//! alternative — guessing that an unknown utility's operands are files — invents
//! rows, which is the failure it cannot.

/// What a utility's positional operands are, once its flags are set aside.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Positionals {
    /// Every operand is a file the utility reads.
    Read,
    /// The first operand is a script or a pattern; the rest are files it reads.
    /// This is the rule that keeps `sed -n '1,220p' src/lib.rs` from reporting
    /// `1,220p` as a file.
    ScriptThenRead,
    /// Every operand but the last is read; the last is written.
    ReadThenWriteLast,
    /// Every operand is a destination.
    Write,
    /// Every operand is removed.
    Delete,
    /// No operand names a file.
    None,
}

pub(crate) struct Utility {
    pub(crate) positionals: Positionals,
    /// Flags that consume the next word as a value rather than a path. Without
    /// these, `head -n 40 src/main.rs` reports a file named `40`.
    pub(crate) value_flags: &'static [&'static str],
    /// Flags that change what the positionals are. Present mostly for the
    /// utilities where a flag supplies the script, so no operand has to:
    /// `sed -e 's/a/b/' one two` reads both operands.
    pub(crate) positional_flags: &'static [(&'static str, Positionals)],
    /// Flags that edit in place, turning read operands into written ones. A
    /// two-character flag also matches its suffixed form, so `-i.bak` counts.
    pub(crate) in_place_flags: &'static [&'static str],
    /// True when this utility runs a script handed to it inline. Its script's
    /// internals are not attributed; redirects around it still are.
    pub(crate) scripting: bool,
}

/// A command that carries another command.
pub(crate) enum Wrapper {
    /// Assignments, flags, and a fixed number of leading operands belong to the
    /// wrapper; what follows is the command it runs.
    Prefix {
        value_flags: &'static [&'static str],
        skip_operands: usize,
    },
    /// A `-c` flag hands a shell one string to read as a command of its own.
    CommandString,
}

const fn utility(positionals: Positionals, value_flags: &'static [&'static str]) -> Utility {
    Utility {
        positionals,
        value_flags,
        positional_flags: &[],
        in_place_flags: &[],
        scripting: false,
    }
}

const READER: Utility = utility(Positionals::Read, &[]);
const HEAD: Utility = utility(
    Positionals::Read,
    &["-n", "-c", "--lines", "--bytes", "--quiet"],
);
const OD: Utility = utility(Positionals::Read, &["-A", "-j", "-N", "-t", "-w", "-S"]);
const SORT: Utility = utility(
    Positionals::Read,
    &[
        "-k",
        "-t",
        "-o",
        "-S",
        "-T",
        "--key",
        "--field-separator",
        "--output",
        "--buffer-size",
        "--temporary-directory",
    ],
);
const CUT: Utility = utility(
    Positionals::Read,
    &[
        "-d",
        "-f",
        "-b",
        "-c",
        "--delimiter",
        "--fields",
        "--bytes",
        "--characters",
        "--output-delimiter",
    ],
);

const SED: Utility = Utility {
    positionals: Positionals::ScriptThenRead,
    value_flags: &["-e", "-f", "--expression", "--file", "-l", "--line-length"],
    positional_flags: &[
        ("-e", Positionals::Read),
        ("--expression", Positionals::Read),
        ("-f", Positionals::Read),
        ("--file", Positionals::Read),
    ],
    in_place_flags: &["-i", "--in-place"],
    scripting: false,
};

const AWK: Utility = Utility {
    positionals: Positionals::ScriptThenRead,
    value_flags: &["-f", "-v", "--file", "--assign", "-F", "--field-separator"],
    positional_flags: &[("-f", Positionals::Read), ("--file", Positionals::Read)],
    in_place_flags: &[],
    scripting: false,
};

const GREP: Utility = Utility {
    positionals: Positionals::ScriptThenRead,
    value_flags: &[
        "-e",
        "-f",
        "-m",
        "-A",
        "-B",
        "-C",
        "-d",
        "-D",
        "--regexp",
        "--file",
        "--max-count",
        "--after-context",
        "--before-context",
        "--context",
        "--include",
        "--exclude",
        "--exclude-dir",
        "--label",
        "--binary-files",
        "--devices",
        "--directories",
        "--color",
        "--colour",
    ],
    positional_flags: &[
        ("-e", Positionals::Read),
        ("--regexp", Positionals::Read),
        ("-f", Positionals::Read),
        ("--file", Positionals::Read),
    ],
    in_place_flags: &[],
    scripting: false,
};

const RIPGREP: Utility = Utility {
    positionals: Positionals::ScriptThenRead,
    value_flags: &[
        "-e",
        "-f",
        "-m",
        "-A",
        "-B",
        "-C",
        "-g",
        "-t",
        "-T",
        "-M",
        "-r",
        "--regexp",
        "--file",
        "--max-count",
        "--after-context",
        "--before-context",
        "--context",
        "--glob",
        "--iglob",
        "--type",
        "--type-not",
        "--max-columns",
        "--max-depth",
        "--replace",
        "--sort",
        "--sortr",
        "--color",
        "--colors",
        "--pre",
    ],
    positional_flags: &[
        ("-e", Positionals::Read),
        ("--regexp", Positionals::Read),
        ("-f", Positionals::Read),
        ("--file", Positionals::Read),
    ],
    in_place_flags: &[],
    scripting: false,
};

const JQ: Utility = Utility {
    positionals: Positionals::ScriptThenRead,
    value_flags: &["-f", "--from-file", "--indent", "--arg", "--argjson"],
    positional_flags: &[
        ("-f", Positionals::Read),
        ("--from-file", Positionals::Read),
    ],
    in_place_flags: &[],
    scripting: false,
};

const TEE: Utility = utility(Positionals::Write, &[]);
const TOUCH: Utility = utility(
    Positionals::Write,
    &["-d", "-t", "-r", "--date", "--time", "--reference"],
);
const TRUNCATE: Utility = utility(
    Positionals::Write,
    &["-s", "-r", "--size", "--reference", "-o", "--io-blocks"],
);

/// `cp` and `mv` read every operand but the last and write the last — unless a
/// target-directory flag names the destination, which makes them all sources.
const COPY: Utility = Utility {
    positionals: Positionals::ReadThenWriteLast,
    value_flags: &["-t", "-S", "--target-directory", "--suffix"],
    positional_flags: &[
        ("-t", Positionals::Read),
        ("--target-directory", Positionals::Read),
    ],
    in_place_flags: &[],
    scripting: false,
};

const REMOVE: Utility = utility(Positionals::Delete, &[]);

/// An inline interpreter. Its positionals are not attributed at all: a script
/// path is not something this crate opens, and script internals are explicitly
/// out of scope. Redirects around the invocation are still literal shell.
const INTERPRETER: Utility = Utility {
    positionals: Positionals::None,
    value_flags: &["-c", "-e", "-E", "-m", "--eval", "--exec", "--command"],
    positional_flags: &[],
    in_place_flags: &[],
    scripting: true,
};

const PREFIX: Wrapper = Wrapper::Prefix {
    value_flags: &[],
    skip_operands: 0,
};

/// The utility this name is known to be, or `None` for the visible
/// under-report.
pub(crate) fn lookup(name: &str) -> Option<&'static Utility> {
    Some(match name {
        "cat" | "tac" | "less" | "more" | "wc" | "nl" | "strings" | "file" | "xxd" | "hexdump"
        | "base64" | "md5sum" | "sha1sum" | "sha256sum" | "sha512sum" | "cksum" | "uniq"
        | "expand" | "unexpand" | "fold" | "paste" | "comm" | "diff" | "diff3" | "cmp"
        | "iconv" | "shasum" => &READER,
        "head" | "tail" => &HEAD,
        "od" => &OD,
        "sort" => &SORT,
        "cut" => &CUT,
        "sed" => &SED,
        "awk" | "gawk" | "nawk" | "mawk" => &AWK,
        "grep" | "egrep" | "fgrep" => &GREP,
        "rg" | "ag" | "ack" => &RIPGREP,
        "jq" | "yq" => &JQ,
        "tee" => &TEE,
        "touch" => &TOUCH,
        "truncate" => &TRUNCATE,
        "cp" | "mv" | "install" | "ln" => &COPY,
        "rm" | "rmdir" | "unlink" | "shred" => &REMOVE,
        "python" | "python3" | "perl" | "ruby" | "node" | "deno" | "bun" | "php" | "lua"
        | "Rscript" | "osascript" => &INTERPRETER,
        _ => return None,
    })
}

/// The wrapper this name is known to be, for the wrapper shapes the corpus
/// actually contains.
pub(crate) fn wrapper(name: &str) -> Option<&'static Wrapper> {
    Some(match name {
        "env" => &Wrapper::Prefix {
            value_flags: &["-u", "-S", "--unset", "--chdir", "-C"],
            skip_operands: 0,
        },
        "sudo" => &Wrapper::Prefix {
            value_flags: &[
                "-u", "-g", "-p", "-C", "-r", "-t", "-U", "-h", "--user", "--group", "--prompt",
                "--role", "--type", "--host",
            ],
            skip_operands: 0,
        },
        "timeout" => &Wrapper::Prefix {
            value_flags: &["-s", "-k", "--signal", "--kill-after"],
            skip_operands: 1,
        },
        "nice" => &Wrapper::Prefix {
            value_flags: &["-n", "--adjustment"],
            skip_operands: 0,
        },
        "xargs" => &Wrapper::Prefix {
            value_flags: &[
                "-n",
                "-P",
                "-I",
                "-i",
                "-d",
                "-L",
                "-s",
                "-E",
                "--max-args",
                "--max-procs",
                "--replace",
                "--delimiter",
                "--max-lines",
                "--max-chars",
            ],
            skip_operands: 0,
        },
        "nohup" | "command" | "stdbuf" | "time" => &PREFIX,
        "sh" | "bash" | "zsh" | "dash" | "ksh" => &Wrapper::CommandString,
        _ => return None,
    })
}
