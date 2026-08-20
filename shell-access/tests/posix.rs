//! What the classifier reports for one POSIX command, exercised through the
//! public API a caller actually has.
//!
//! The shapes here are the ones a naive walk gets confidently wrong: a script
//! operand read as a file, a file descriptor read as a path, a word after a
//! redirect read as a second destination.

use shell_access::{
    AccessOperation::{self, Delete, Read, Write},
    AmbiguityReason, ClassificationError, Classifier, FileAccess, ShellLanguage,
};

/// Every access one command produced, or a panic naming the error, since a test
/// that silently swallowed the error would assert nothing.
fn accesses(command: &str) -> Vec<FileAccess> {
    Classifier::POSIX
        .classify(command)
        .collect::<Result<Vec<_>, _>>()
        .expect("the command parses")
}

/// The `(operation, path)` pairs a command produced, which is what most of these
/// assertions are about.
fn attributed(command: &str) -> Vec<(AccessOperation, String)> {
    accesses(command)
        .into_iter()
        .map(|access| (access.op, access.path))
        .collect()
}

#[test]
fn classifies_literal_reader_and_redirects() {
    let classifier = Classifier::POSIX.with_cwd("/work/app");

    let accesses: Vec<_> = classifier
        .classify("cat config/base.yml >> build/out.env")
        .collect::<Result<Vec<_>, _>>()
        .expect("the command parses");

    assert_eq!(
        accesses,
        [
            FileAccess {
                op: Read,
                path: "config/base.yml".to_string(),
                cwd: Some("/work/app".into()),
                ambiguity: None,
                scripting: false,
            },
            FileAccess {
                op: Write,
                path: "build/out.env".to_string(),
                cwd: Some("/work/app".into()),
                ambiguity: None,
                scripting: false,
            },
        ]
    );
}

/// The regression an early probe produced 1,066 times: the parse succeeded and
/// reported `1,220p` as a file with complete confidence.
#[test]
fn a_script_operand_is_not_a_file() {
    assert_eq!(
        attributed("sed -n '1,220p' src/lib.rs"),
        [(Read, "src/lib.rs".to_string())]
    );
}

/// The other confident falsehood from the same probe: `2` became a top write
/// destination corpus-wide.
#[test]
fn a_file_descriptor_is_not_a_path() {
    assert_eq!(
        attributed("rg needle src/lib.rs 2>&1"),
        [(Read, "src/lib.rs".to_string())]
    );
    assert_eq!(
        attributed("cargo build 2> build/build.err"),
        [(Write, "build/build.err".to_string())]
    );
}

/// tree-sitter-bash issue #233: `file_redirect` takes repeated destinations, so
/// the grammar itself offers `'done'` as a second one.
#[test]
fn a_word_after_a_redirect_destination_is_not_another_destination() {
    assert_eq!(
        attributed("printf > build/out.env 'done'"),
        [(Write, "build/out.env".to_string())]
    );
}

#[test]
fn reports_deletes_distinctly_from_writes() {
    assert_eq!(
        attributed("rm -f build/stale.env"),
        [(Delete, "build/stale.env".to_string())]
    );
}

#[test]
fn classifies_each_pipeline_stage_independently() {
    assert_eq!(
        attributed("cat config/base.yml | sed -n '1,10p' | tee build/head.env"),
        [
            (Read, "config/base.yml".to_string()),
            (Write, "build/head.env".to_string()),
        ]
    );
}

#[test]
fn peels_wrappers_down_to_the_command_they_run() {
    assert_eq!(
        attributed("sudo -u builder rm -f build/stale.env"),
        [(Delete, "build/stale.env".to_string())]
    );
    assert_eq!(
        attributed("env RUST_LOG=debug cat config/base.yml"),
        [(Read, "config/base.yml".to_string())]
    );
    assert_eq!(
        attributed("bash -lc 'cat config/base.yml >> build/out.env'"),
        [
            (Read, "config/base.yml".to_string()),
            (Write, "build/out.env".to_string()),
        ]
    );
}

#[test]
fn copies_read_every_operand_but_the_last_and_write_the_last() {
    assert_eq!(
        attributed("cp config/base.yml build/base.yml"),
        [
            (Read, "config/base.yml".to_string()),
            (Write, "build/base.yml".to_string()),
        ]
    );
}

#[test]
fn an_in_place_flag_turns_a_read_operand_into_a_write() {
    assert_eq!(
        attributed("sed -i 's/alpha/beta/' src/main.rs"),
        [(Write, "src/main.rs".to_string())]
    );
}

/// An unresolvable operand keeps the same `{op, path}` shape as a literal one.
/// It is neither dropped nor promoted to a path; the reason is what tells the
/// two apart.
#[test]
fn unresolvable_operands_stay_visible_with_their_reason() {
    let glob = accesses("cat logs/*.txt");
    assert_eq!(glob[0].op, Read);
    assert_eq!(glob[0].path, "logs/*.txt");
    assert_eq!(glob[0].ambiguity, Some(AmbiguityReason::Glob));

    let expansion = accesses("cat \"$CONFIG_DIR/base.yml\"");
    assert_eq!(expansion[0].path, "$CONFIG_DIR/base.yml");
    assert_eq!(expansion[0].ambiguity, Some(AmbiguityReason::Expansion));

    let substitution = accesses("wc -l $(git ls-files)");
    assert_eq!(substitution[0].path, "$(git ls-files)");
    assert_eq!(
        substitution[0].ambiguity,
        Some(AmbiguityReason::CommandSubstitution)
    );
}

/// An unquoted delimiter means the shell expanded the body, so nothing in it can
/// be read literally. A quoted delimiter leaves the body alone, and a literal
/// body names no file.
#[test]
fn an_unquoted_heredoc_body_is_ambiguous_and_a_quoted_one_is_not() {
    let expanded = accesses("cat <<EOF > build/out.env\ngenerated for $TARGET\nEOF\n");
    assert_eq!(
        expanded
            .iter()
            .map(|access| (access.op, access.path.as_str(), access.ambiguity))
            .collect::<Vec<_>>(),
        [
            (Read, "<<EOF", Some(AmbiguityReason::ExpandedHeredoc)),
            (Write, "build/out.env", None),
        ]
    );

    let literal = accesses("cat <<'LIMIT' > build/out.env\ngenerated for $TARGET\nLIMIT\n");
    assert_eq!(attributed_of(&literal), [(Write, "build/out.env")]);
}

fn attributed_of(accesses: &[FileAccess]) -> Vec<(AccessOperation, &str)> {
    accesses
        .iter()
        .map(|access| (access.op, access.path.as_str()))
        .collect()
}

#[test]
fn stdin_only_readers_and_unknown_utilities_report_nothing() {
    assert_eq!(attributed("cat"), []);
    assert_eq!(attributed("cat config/base.yml | wc -l").len(), 1);
    assert_eq!(attributed("git diff --stat config/base.yml"), []);
    assert_eq!(attributed("mkdir -p build/logs"), []);
}

/// An inline interpreter's script is not attributed. The literal redirect around
/// it is, and it says it belongs to a scripted invocation.
#[test]
fn an_inline_interpreter_is_scripting_and_its_redirects_are_still_recorded() {
    let accesses = accesses("python3 -c 'print(1)' > build/out.env");
    assert_eq!(attributed_of(&accesses), [(Write, "build/out.env")]);
    assert!(accesses[0].scripting);

    assert_eq!(
        attributed("python3 -c 'open(\"build/out.env\", \"w\")'"),
        []
    );
    assert_eq!(attributed("python3 scripts/report.py"), []);
}

#[test]
fn a_compound_command_reports_both_of_its_commands_in_order() {
    assert_eq!(
        attributed("cat config/base.yml && rm -f build/stale.env"),
        [
            (Read, "config/base.yml".to_string()),
            (Delete, "build/stale.env".to_string()),
        ]
    );
}

/// A fragment the grammar cannot read is an error at its place in the stream,
/// not an "ambiguous" access and not silence.
#[test]
fn unparsable_text_is_an_error_rather_than_a_partial_attribution() {
    let records: Vec<_> = Classifier::POSIX.classify("cat 'config/base.yml").collect();
    assert_eq!(records, [Err(ClassificationError::Parse)]);
}

/// A language this crate does not implement is refused rather than read under
/// POSIX rules, so a later fish or PowerShell implementation changes nothing a
/// caller already consumes.
#[test]
fn an_unimplemented_language_is_refused_rather_than_read_as_posix() {
    let records: Vec<_> = Classifier::new()
        .language(ShellLanguage::Unsupported("powershell".to_string()))
        .classify("Get-Content config/base.yml")
        .collect();
    assert_eq!(
        records,
        [Err(ClassificationError::UnsupportedLanguage {
            language: "powershell".to_string()
        })]
    );

    let detected: Vec<_> = Classifier::new()
        .classify("#!/usr/bin/env fish\ncat config/base.yml")
        .collect();
    assert_eq!(
        detected,
        [Err(ClassificationError::UnsupportedLanguage {
            language: "fish".to_string()
        })]
    );
}

/// The recorded directory is context, copied onto each access and never joined
/// onto the operand. A relative path stays relative.
#[test]
fn a_recorded_directory_is_never_joined_onto_an_operand() {
    let access = &Classifier::POSIX
        .with_cwd("/work/other")
        .classify("cat notes/todo.md")
        .collect::<Result<Vec<_>, _>>()
        .expect("the command parses")[0];

    assert_eq!(access.path, "notes/todo.md");
    assert_eq!(
        access.cwd.as_deref(),
        Some(std::path::Path::new("/work/other"))
    );
}
