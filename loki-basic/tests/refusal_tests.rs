// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The "never" list (macro spec §7): one assertion per refused surface. Every
//! refusal must be an **untrappable** feature-refusal (error 1004) that
//! `On Error Resume Next` cannot swallow (spec §7, threat T2/T3/T5).

use loki_basic::host::NullHost;
use loki_basic::parser::Parser;
use loki_basic::{BasicError, Dialect, Interp, Value};

/// Runs `Sub S()` in `src` and returns the error it raises (expected).
fn run_err(src: &str) -> BasicError {
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    let mut interp = Interp::new(&module, NullHost).expect("new");
    interp.call("S", Vec::new()).expect_err("expected refusal")
}

/// Asserts that invoking `stmt` raises an untrappable feature-refusal (1004).
fn assert_refused(stmt: &str) {
    let src = format!("Sub S()\n {stmt}\nEnd Sub");
    match run_err(&src) {
        BasicError::Runtime(e) => {
            assert_eq!(e.number, 1004, "wrong error for `{stmt}`: {e:?}");
            assert!(!e.trappable, "`{stmt}` must be untrappable");
        }
        other => panic!("expected runtime refusal for `{stmt}`, got {other:?}"),
    }
}

#[test]
fn process_execution_refused() {
    assert_refused("Shell \"calc.exe\"");
    assert_refused("SendKeys \"%{F4}\"");
    assert_refused("AppActivate \"Notepad\"");
}

#[test]
fn com_ole_automation_refused() {
    assert_refused("Dim o\n Set o = CreateObject(\"WScript.Shell\")");
    assert_refused("Dim o\n Set o = GetObject(\"winmgmts:\")");
}

#[test]
fn uno_service_manager_refused() {
    assert_refused("Dim o\n Set o = createUnoService(\"com.sun.star.bridge.UnoUrlResolver\")");
}

#[test]
fn path_addressed_file_io_refused() {
    assert_refused("Kill \"C:\\\\secret.txt\"");
    assert_refused("MkDir \"C:\\\\evil\"");
    assert_refused("RmDir \"C:\\\\evil\"");
    assert_refused("FileCopy \"a\", \"b\"");
    assert_refused("SetAttr \"a\", 0");
    assert_refused("Dim d\n d = Dir(\"C:\\\\*.*\")");
}

#[test]
fn registry_and_environment_refused() {
    assert_refused("SaveSetting \"App\", \"S\", \"K\", \"V\"");
    assert_refused("Dim v\n v = GetSetting(\"App\", \"S\", \"K\")");
    assert_refused("Dim p\n p = Environ(\"PATH\")");
}

#[test]
fn dde_refused() {
    assert_refused("Dim c\n c = DDEInitiate(\"Excel\", \"Sheet1\")");
}

#[test]
fn timer_background_execution_refused() {
    assert_refused("Application.OnTime Now, \"Evil\"");
    assert_refused("Dim t\n t = Timer");
}

#[test]
fn ffi_declare_call_refused() {
    // A `Declare … Lib` FFI import parses, but calling it is refused by name.
    let src = "Declare Function GetTickCount Lib \"kernel32\" () As Long\n\
               Sub S()\n Dim n As Long\n n = GetTickCount()\nEnd Sub";
    match run_err(src) {
        BasicError::Runtime(e) => {
            assert_eq!(e.number, 1004);
            assert!(!e.trappable);
        }
        other => panic!("expected FFI refusal, got {other:?}"),
    }
}

#[test]
fn new_external_object_refused() {
    assert_refused("Dim o\n Set o = New Something");
}

#[test]
fn refusal_survives_on_error_resume_next() {
    // The whole point of untrappable refusals (spec §7): a malicious macro
    // cannot `On Error Resume Next` its way past `Shell`.
    let src = "Sub S()\n On Error Resume Next\n Shell \"calc.exe\"\nEnd Sub";
    match run_err(src) {
        BasicError::Runtime(e) => assert!(!e.trappable && e.number == 1004),
        other => panic!("On Error must not swallow a refusal, got {other:?}"),
    }
}

#[test]
fn ordinary_undefined_call_is_a_normal_error_not_a_refusal() {
    // A plain unknown Sub is error 35 (trappable), not a feature refusal — the
    // refusal path must not swallow every unknown name.
    let src = "Sub S()\n NotARealSub 1, 2\nEnd Sub";
    match run_err(src) {
        BasicError::Runtime(e) => {
            assert_eq!(e.number, 35);
            assert!(e.trappable);
        }
        other => panic!("expected error 35, got {other:?}"),
    }
}

#[test]
fn refused_names_are_not_usable_as_values_either() {
    // Even reading a refused name as a bare value refuses (not silently Empty).
    let src = "Function F()\n F = Timer\nEnd Function";
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    let mut interp = Interp::new(&module, NullHost).expect("new");
    match interp.call("F", Vec::new()) {
        Err(BasicError::Runtime(e)) => assert_eq!(e.number, 1004),
        other => panic!("expected refusal reading Timer, got {other:?}"),
    }
    let _ = Value::Empty;
}
