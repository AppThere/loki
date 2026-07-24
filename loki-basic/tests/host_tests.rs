// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Execution-surface tests (macro spec Phase 5): object-model dispatch through
//! the [`Host`] seam, `MsgBox`/`InputBox` routing, and the "never"-list
//! refusals. A small mock host stands in for `loki-macro-host`'s real broker.

use loki_basic::host::{DialogKind, DialogRequest, FuelVerdict, Host, ObjectRef};
use loki_basic::parser::Parser;
use loki_basic::{BasicError, Dialect, Interp, RuntimeError, Value};

/// A mock host exposing an `Application` root and a mutable `Document` with a
/// `Text` property and an `AppendText` method — enough to exercise property
/// get/set, method calls, `Set`/`Is`, and `With`.
#[derive(Default)]
struct MockHost {
    text: String,
    /// Records every dialog the macro raised, for assertions.
    dialogs: Vec<DialogRequest>,
    /// When true, the host denies dialogs (models a missing `UiDialog` grant).
    deny_dialogs: bool,
}

const APP: ObjectRef = ObjectRef(1);
const DOC: ObjectRef = ObjectRef(2);

impl Host for MockHost {
    fn consume_fuel(&mut self, _units: u64) -> FuelVerdict {
        FuelVerdict::Continue
    }

    fn get_root(&mut self, name: &str) -> Option<ObjectRef> {
        match name.to_ascii_lowercase().as_str() {
            "application" => Some(APP),
            "activedocument" | "thisdocument" | "thiscomponent" => Some(DOC),
            _ => None,
        }
    }

    fn get_member(
        &mut self,
        obj: ObjectRef,
        name: &str,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        match (obj, name.to_ascii_lowercase().as_str()) {
            (APP, "name") => Ok(Value::Str("Loki".into())),
            (APP, "activedocument") => Ok(Value::Object(DOC)),
            (DOC, "name") => Ok(Value::Str("Doc".into())),
            (DOC, "text") => Ok(Value::Str(self.text.clone())),
            (DOC, "self") => Ok(Value::Object(DOC)),
            (DOC, "appendtext") => {
                let s = args
                    .first()
                    .cloned()
                    .unwrap_or(Value::Empty)
                    .to_basic_string()?;
                self.text.push_str(&s);
                Ok(Value::Empty)
            }
            _ => Err(RuntimeError::new(438, "no member")),
        }
    }

    fn set_member(&mut self, obj: ObjectRef, name: &str, value: Value) -> Result<(), RuntimeError> {
        match (obj, name.to_ascii_lowercase().as_str()) {
            (DOC, "text") => {
                self.text = value.to_basic_string()?;
                Ok(())
            }
            _ => Err(RuntimeError::new(438, "no member")),
        }
    }

    fn dialog(&mut self, req: &DialogRequest) -> Result<Value, RuntimeError> {
        if self.deny_dialogs {
            return Err(RuntimeError::new(70, "Permission denied"));
        }
        self.dialogs.push(req.clone());
        match req.kind {
            DialogKind::Message => Ok(Value::Int(1)), // vbOK
            DialogKind::Input => Ok(Value::Str(req.default.clone().unwrap_or_default())),
        }
    }
}

fn run_host(src: &str, proc: &str, host: MockHost) -> (Result<Value, BasicError>, MockHost) {
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    let mut interp = Interp::new(&module, host).expect("new");
    let r = interp.call(proc, Vec::new());
    // Recover the host to inspect recorded effects.
    let host = interp.into_host();
    (r, host)
}

// ── Object model ─────────────────────────────────────────────────────────────

#[test]
fn reads_a_property_off_a_root() {
    let src = "Function F() As String\n F = Application.Name\nEnd Function";
    let (r, _) = run_host(src, "F", MockHost::default());
    assert_eq!(r.unwrap(), Value::Str("Loki".into()));
}

#[test]
fn calls_a_method_that_mutates_the_document() {
    let src =
        "Sub Go()\n ActiveDocument.AppendText \"hi\"\n ActiveDocument.AppendText \"!\"\nEnd Sub";
    let (r, host) = run_host(src, "Go", MockHost::default());
    r.unwrap();
    assert_eq!(host.text, "hi!");
}

#[test]
fn assigns_a_property() {
    let src = "Sub Go()\n ActiveDocument.Text = \"replaced\"\nEnd Sub";
    let (r, host) = run_host(src, "Go", MockHost::default());
    r.unwrap();
    assert_eq!(host.text, "replaced");
}

#[test]
fn set_binds_an_object_and_chains_members() {
    let src = "Function F() As String\n Dim d As Object\n Set d = Application.ActiveDocument\n \
               d.AppendText \"x\"\n F = d.Text\nEnd Function";
    let (r, _) = run_host(src, "F", MockHost::default());
    assert_eq!(r.unwrap(), Value::Str("x".into()));
}

#[test]
fn is_operator_compares_object_identity() {
    let src = "Function F() As Boolean\n Dim a As Object\n Set a = ActiveDocument\n \
               F = a Is ActiveDocument.Self\nEnd Function";
    let (r, _) = run_host(src, "F", MockHost::default());
    assert_eq!(r.unwrap(), Value::Bool(true));
}

#[test]
fn is_nothing_is_true_for_unset_object() {
    let src = "Function F() As Boolean\n Dim a As Object\n F = a Is Nothing\nEnd Function";
    let (r, _) = run_host(src, "F", MockHost::default());
    assert_eq!(r.unwrap(), Value::Bool(true));
}

#[test]
fn with_block_resolves_the_receiver() {
    let src = "Sub Go()\n With ActiveDocument\n .AppendText \"a\"\n .AppendText \"b\"\n \
               End With\nEnd Sub";
    let (r, host) = run_host(src, "Go", MockHost::default());
    r.unwrap();
    assert_eq!(host.text, "ab");
}

// ── Dialogs ──────────────────────────────────────────────────────────────────

#[test]
fn msgbox_routes_to_the_host_and_returns_a_code() {
    let src = "Function F() As Integer\n F = MsgBox(\"hello\", 4, \"Title\")\nEnd Function";
    let (r, host) = run_host(src, "F", MockHost::default());
    assert_eq!(r.unwrap(), Value::Int(1));
    assert_eq!(host.dialogs.len(), 1);
    assert_eq!(host.dialogs[0].prompt, "hello");
    assert_eq!(host.dialogs[0].buttons, 4);
    assert_eq!(host.dialogs[0].title.as_deref(), Some("Title"));
}

#[test]
fn msgbox_as_statement_routes_too() {
    let src = "Sub Go()\n MsgBox \"note\"\nEnd Sub";
    let (r, host) = run_host(src, "Go", MockHost::default());
    r.unwrap();
    assert_eq!(host.dialogs.len(), 1);
    assert_eq!(host.dialogs[0].kind, DialogKind::Message);
}

#[test]
fn inputbox_returns_default_from_host() {
    let src = "Function F() As String\n F = InputBox(\"q\", \"t\", \"preset\")\nEnd Function";
    let (r, _) = run_host(src, "F", MockHost::default());
    assert_eq!(r.unwrap(), Value::Str("preset".into()));
}

#[test]
fn denied_dialog_is_a_trappable_error() {
    // A denied capability surfaces as a *trappable* error, so a well-written
    // macro can `On Error` around it (spec §5.1).
    let src = "Function F() As Integer\n On Error Resume Next\n F = 7\n \
               F = MsgBox(\"x\")\n If Err.Number <> 0 Then F = 99\nEnd Function";
    let host = MockHost {
        deny_dialogs: true,
        ..Default::default()
    };
    let (r, _) = run_host(src, "F", host);
    assert_eq!(r.unwrap(), Value::Int(99));
}
