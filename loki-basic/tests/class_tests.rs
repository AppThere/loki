// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Class-module tests (macro spec §4.2, phase 6): `New`, fields, methods,
//! `Property Get/Let/Set`, `Me`, and reference semantics. A class instance is
//! pure interpreter heap — these run against a [`NullHost`], proving no host
//! authority is involved.

use loki_basic::host::NullHost;
use loki_basic::parser::Parser;
use loki_basic::{Dialect, Interp, Value};

/// Runs a whole module (which defines a class + a `Sub`/`Function` entry point)
/// and returns the entry point's result.
fn run(src: &str, entry: &str) -> Value {
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    let mut interp = Interp::new(&module, NullHost).expect("new");
    interp.call(entry, Vec::new()).expect("call")
}

fn run_err(src: &str, entry: &str) -> loki_basic::BasicError {
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    let mut interp = Interp::new(&module, NullHost).expect("new");
    interp.call(entry, Vec::new()).expect_err("expected error")
}

const COUNTER: &str = "\
Class Counter
    Private mCount As Long
    Public Sub Bump()
        mCount = mCount + 1
    End Sub
    Public Function Value() As Long
        Value = mCount
    End Function
End Class
";

#[test]
fn new_creates_an_instance_and_methods_mutate_it() {
    let src = format!(
        "{COUNTER}
Function F() As Long
    Dim c
    Set c = New Counter
    c.Bump
    c.Bump
    c.Bump
    F = c.Value
End Function"
    );
    assert_eq!(run(&src, "F"), Value::Long(3));
}

#[test]
fn fields_are_read_and_written_through_dot_access() {
    let src = "\
Class Point
    Public X As Long
    Public Y As Long
End Class
Function F() As Long
    Dim p
    Set p = New Point
    p.X = 10
    p.Y = 32
    F = p.X + p.Y
End Function";
    // Field assignment stores the literal's own type (no coerce-to-declared, as
    // with local `Dim`s), so `Int + Int` stays `Int`.
    assert_eq!(run(src, "F"), Value::Int(42));
}

#[test]
fn property_get_let_pair_round_trips() {
    let src = "\
Class Temperature
    Private mC As Double
    Public Property Get Celsius() As Double
        Celsius = mC
    End Property
    Public Property Let Celsius(ByVal v As Double)
        mC = v
    End Property
    Public Property Get Fahrenheit() As Double
        Fahrenheit = mC * 9 / 5 + 32
    End Property
End Class
Function F() As Double
    Dim t
    Set t = New Temperature
    t.Celsius = 100
    F = t.Fahrenheit
End Function";
    assert_eq!(run(src, "F"), Value::Double(212.0));
}

#[test]
fn me_and_implicit_member_access_resolve_to_the_instance() {
    // `Describe` reads a field bare, and calls a sibling method bare, and via Me.
    let src = "\
Class Greeter
    Public Name As String
    Public Function Greeting() As String
        Greeting = \"Hello, \" & Name
    End Function
    Public Function Loud() As String
        Loud = Me.Greeting & \"!\"
    End Function
End Class
Function F() As String
    Dim g
    Set g = New Greeter
    g.Name = \"Ada\"
    F = g.Loud
End Function";
    assert_eq!(run(src, "F"), Value::Str("Hello, Ada!".to_string()));
}

#[test]
fn set_shares_the_same_instance_by_reference() {
    let src = format!(
        "{COUNTER}
Function F() As Long
    Dim a
    Dim b
    Set a = New Counter
    Set b = a
    a.Bump
    b.Bump
    F = a.Value
End Function"
    );
    // Both handles reference one instance → two bumps visible through `a`.
    assert_eq!(run(&src, "F"), Value::Long(2));
}

#[test]
fn two_instances_are_independent() {
    let src = format!(
        "{COUNTER}
Function F() As Long
    Dim a
    Dim b
    Set a = New Counter
    Set b = New Counter
    a.Bump
    a.Bump
    b.Bump
    F = a.Value * 10 + b.Value
End Function"
    );
    assert_eq!(run(&src, "F"), Value::Long(21));
}

#[test]
fn is_operator_compares_instance_identity() {
    let src = format!(
        "{COUNTER}
Function F() As Boolean
    Dim a
    Dim b
    Set a = New Counter
    Set b = a
    F = (a Is b)
End Function"
    );
    assert_eq!(run(&src, "F"), Value::Bool(true));
}

#[test]
fn method_call_with_arguments_binds_positionally() {
    let src = "\
Class Adder
    Private mBase As Long
    Public Sub Init(ByVal b As Long)
        mBase = b
    End Sub
    Public Function Plus(ByVal n As Long) As Long
        Plus = mBase + n
    End Function
End Class
Function F() As Long
    Dim a
    Set a = New Adder
    a.Init 100
    F = a.Plus(5)
End Function";
    assert_eq!(run(src, "F"), Value::Int(105));
}

#[test]
fn new_of_unknown_class_is_refused_untrappably() {
    // `New Something` where Something is not a user class is an external object
    // (COM/ProgID) — on the "never" list (§7), refused and untrappable.
    let src = "\
Function F() As Long
    On Error Resume Next
    Dim x
    Set x = New Scripting.Dictionary
    F = 7
End Function";
    match run_err(src, "F") {
        loki_basic::BasicError::Runtime(e) => {
            assert_eq!(e.number, 1004, "expected feature refusal");
            assert!(!e.trappable, "refusal must be untrappable");
        }
        other => panic!("expected runtime refusal, got {other:?}"),
    }
}

#[test]
fn unknown_member_is_error_438() {
    let src = format!(
        "{COUNTER}
Function F() As Long
    Dim c
    Set c = New Counter
    F = c.NoSuchThing
End Function"
    );
    match run_err(&src, "F") {
        loki_basic::BasicError::Runtime(e) => assert_eq!(e.number, 438),
        other => panic!("expected 438, got {other:?}"),
    }
}
