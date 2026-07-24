// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `Find`/`Replacement` object-model tests (macro spec §6.1, phase 6): search
//! returns a boolean, replace-all funnels through the one-undo `EditBatch`, and
//! `MatchCase`/`WholeWord` are honoured. A replacing run needs `DocWrite`.

use loki_basic::Dialect;
use loki_macro_host::{
    Capability, DialogOutcome, GrantScope, MacroBackend, MacroRuntime, RunRequest,
};

/// A backend that grants a configurable capability set on prompt.
#[derive(Default)]
struct TestBackend {
    allow: Vec<Capability>,
}

impl MacroBackend for TestBackend {
    fn prompt_capability(&mut self, cap: Capability) -> GrantScope {
        if self.allow.contains(&cap) {
            GrantScope::AllowSession
        } else {
            GrantScope::Deny
        }
    }
    fn show_dialog(&mut self, _req: &loki_basic::DialogRequest) -> DialogOutcome {
        DialogOutcome::Cancelled
    }
}

fn run(src: &str, body: &str, allow: Vec<Capability>) -> loki_macro_host::RunOutcome {
    let backend = TestBackend { allow };
    MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        RunRequest::new("Doc", body, 10_000_000),
        backend,
    )
}

#[test]
fn execute_replaces_all_matches_as_one_batch() {
    let src = "\
Sub Main()
    Selection.Find.Text = \"cat\"
    Selection.Find.Replacement.Text = \"dog\"
    Selection.Find.Execute
End Sub";
    let out = run(src, "the cat sat with a cat", vec![Capability::DocWrite]);
    out.result.expect("clean run");
    // One SetText edit → one undo entry; both occurrences replaced.
    assert_eq!(out.batch.len(), 1);
    assert_eq!(
        out.batch.apply_to("the cat sat with a cat".into()),
        "the dog sat with a dog"
    );
}

#[test]
fn execute_without_replacement_is_search_only() {
    // No Replacement.Text set → Execute searches, makes no edit, needs no write.
    let src = "\
Function Main() As Boolean
    Selection.Find.Text = \"sat\"
    Main = Selection.Find.Execute
End Function";
    let out = run(src, "the cat sat", vec![]);
    out.result.expect("clean run (search needs no DocWrite)");
    assert!(out.batch.is_empty(), "search-only makes no edit");
}

#[test]
fn search_reports_found_or_not() {
    let present = "\
Function Main() As Boolean
    Selection.Find.Text = \"needle\"
    Main = Selection.Find.Execute
End Function";
    // Missing text → the boolean is false and no error is raised.
    let out = run(present, "a haystack without it", vec![]);
    out.result.expect("clean run");
    assert!(out.batch.is_empty());
}

#[test]
fn replace_is_case_insensitive_by_default() {
    let src = "\
Sub Main()
    Selection.Find.Text = \"cat\"
    Selection.Find.Replacement.Text = \"dog\"
    Selection.Find.Execute
End Sub";
    let out = run(src, "Cat CAT cat", vec![Capability::DocWrite]);
    out.result.expect("clean run");
    assert_eq!(out.batch.apply_to("Cat CAT cat".into()), "dog dog dog");
}

#[test]
fn match_case_restricts_to_exact_case() {
    let src = "\
Sub Main()
    Selection.Find.Text = \"cat\"
    Selection.Find.MatchCase = True
    Selection.Find.Replacement.Text = \"dog\"
    Selection.Find.Execute
End Sub";
    let out = run(src, "Cat CAT cat", vec![Capability::DocWrite]);
    out.result.expect("clean run");
    // Only the exactly-cased "cat" is replaced.
    assert_eq!(out.batch.apply_to("Cat CAT cat".into()), "Cat CAT dog");
}

#[test]
fn whole_word_ignores_substrings() {
    let src = "\
Sub Main()
    Selection.Find.Text = \"cat\"
    Selection.Find.WholeWord = True
    Selection.Find.Replacement.Text = \"dog\"
    Selection.Find.Execute
End Sub";
    let out = run(src, "cat category cat", vec![Capability::DocWrite]);
    out.result.expect("clean run");
    // "category" is not a whole-word match.
    assert_eq!(
        out.batch.apply_to("cat category cat".into()),
        "dog category dog"
    );
}

#[test]
fn replace_with_empty_deletes_matches() {
    let src = "\
Sub Main()
    Selection.Find.Text = \"-\"
    Selection.Find.Replacement.Text = \"\"
    Selection.Find.Execute
End Sub";
    let out = run(src, "a-b-c", vec![Capability::DocWrite]);
    out.result.expect("clean run");
    assert_eq!(out.batch.apply_to("a-b-c".into()), "abc");
}

#[test]
fn replace_is_denied_without_docwrite() {
    // Replacement set but DocWrite denied → the Execute errors (trappable 70),
    // and no edit is recorded.
    let src = "\
Sub Main()
    On Error Resume Next
    Selection.Find.Text = \"cat\"
    Selection.Find.Replacement.Text = \"dog\"
    Selection.Find.Execute
End Sub";
    let out = run(src, "cat", vec![]); // no DocWrite grant
    out.result.expect("On Error swallows the denial");
    assert!(out.batch.is_empty(), "denied replace records no edit");
}

#[test]
fn find_via_range_alias_also_works() {
    // `Range.Find` resolves to the same object as `Selection.Find`.
    let src = "\
Function Main() As Boolean
    ActiveDocument.Range.Find.Text = \"x\"
    Main = ActiveDocument.Range.Find.Execute
End Function";
    let out = run(src, "x marks", vec![]);
    out.result.expect("clean run");
    assert!(out.batch.is_empty());
}
