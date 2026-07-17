// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end interpreter tests: parse a module, call a procedure, check the
//! result. These double as the Phase 2 conformance suite (dialect-shared).

use loki_basic::host::FuelBudget;
use loki_basic::parser::Parser;
use loki_basic::{Dialect, Interp, Value};

/// Runs `Function F(...)` (or a `Sub`) in `src`, returning its result.
fn run(src: &str, func: &str, args: Vec<Value>) -> Value {
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    // Generous fuel; individual tests that probe the cap set their own budget.
    let mut interp = Interp::new(&module, FuelBudget::new(10_000_000)).expect("new");
    interp.call(func, args).expect("call")
}

fn run_err(src: &str, func: &str) -> loki_basic::BasicError {
    let module = Parser::parse_module(src, Dialect::Vba).expect("parse");
    let mut interp = Interp::new(&module, FuelBudget::new(10_000_000)).expect("new");
    interp.call(func, Vec::new()).expect_err("expected error")
}

#[test]
fn function_returns_via_name() {
    let v = run("Function F()\n F = 40 + 2\nEnd Function", "F", vec![]);
    assert_eq!(v, Value::Int(42));
}

#[test]
fn parameters_and_arithmetic() {
    let src = "Function Add(a As Long, b As Long) As Long\n Add = a + b\nEnd Function";
    assert_eq!(
        run(src, "Add", vec![Value::Long(3), Value::Long(4)]),
        Value::Long(7)
    );
}

#[test]
fn if_elseif_else() {
    let src = "Function Grade(n)\n If n >= 90 Then\n Grade = \"A\"\n ElseIf n >= 80 Then\n Grade = \"B\"\n Else\n Grade = \"C\"\n End If\nEnd Function";
    assert_eq!(
        run(src, "Grade", vec![Value::Int(85)]),
        Value::Str("B".into())
    );
    assert_eq!(
        run(src, "Grade", vec![Value::Int(50)]),
        Value::Str("C".into())
    );
}

#[test]
fn for_loop_sums() {
    let src = "Function Sum(n)\n Dim i, t\n t = 0\n For i = 1 To n\n t = t + i\n Next i\n Sum = t\nEnd Function";
    assert_eq!(run(src, "Sum", vec![Value::Int(10)]), Value::Int(55));
}

#[test]
fn for_step_down() {
    let src = "Function Countdown()\n Dim i, s\n s = \"\"\n For i = 3 To 1 Step -1\n s = s & i\n Next\n Countdown = s\nEnd Function";
    assert_eq!(run(src, "Countdown", vec![]), Value::Str("321".into()));
}

#[test]
fn do_while_loop() {
    let src = "Function F()\n Dim n, c\n n = 8 : c = 0\n Do While n > 1\n n = n \\ 2\n c = c + 1\n Loop\n F = c\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(3));
}

#[test]
fn recursion_factorial() {
    let src = "Function Fact(n)\n If n <= 1 Then\n Fact = 1\n Else\n Fact = n * Fact(n - 1)\n End If\nEnd Function";
    assert_eq!(run(src, "Fact", vec![Value::Int(5)]), Value::Int(120));
}

#[test]
fn select_case() {
    let src = "Function Name(n)\n Select Case n\n Case 1\n Name = \"one\"\n Case 2, 3\n Name = \"few\"\n Case Is >= 10\n Name = \"many\"\n Case Else\n Name = \"other\"\n End Select\nEnd Function";
    assert_eq!(
        run(src, "Name", vec![Value::Int(1)]),
        Value::Str("one".into())
    );
    assert_eq!(
        run(src, "Name", vec![Value::Int(3)]),
        Value::Str("few".into())
    );
    assert_eq!(
        run(src, "Name", vec![Value::Int(99)]),
        Value::Str("many".into())
    );
    assert_eq!(
        run(src, "Name", vec![Value::Int(4)]),
        Value::Str("other".into())
    );
}

#[test]
fn arrays_dim_and_index() {
    // Note: declared numeric element types are not coerced in Phase 2, so the
    // stored elements keep the literal's `Integer` type.
    let src = "Function F()\n Dim a(1 To 3) As Long\n a(1) = 10 : a(2) = 20 : a(3) = 30\n F = a(1) + a(2) + a(3)\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(60));
}

#[test]
fn for_each_over_array() {
    let src = "Function F()\n Dim a(1 To 3), x, t\n a(1) = 5 : a(2) = 7 : a(3) = 9\n t = 0\n For Each x In a\n t = t + x\n Next\n F = t\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(21));
}

#[test]
fn byref_modifies_caller() {
    let src = "Sub Inc(ByRef x)\n x = x + 1\nEnd Sub\nFunction F()\n Dim n\n n = 5\n Inc n\n F = n\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(6));
}

#[test]
fn byval_does_not_modify_caller() {
    let src = "Sub Inc(ByVal x)\n x = x + 1\nEnd Sub\nFunction F()\n Dim n\n n = 5\n Inc n\n F = n\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(5));
}

#[test]
fn builtins_string_and_math() {
    assert_eq!(
        run(
            "Function F()\n F = UCase(Left(\"hello\", 3))\nEnd Function",
            "F",
            vec![]
        ),
        Value::Str("HEL".into())
    );
    assert_eq!(
        run(
            "Function F()\n F = Abs(-7) + Len(\"abcd\")\nEnd Function",
            "F",
            vec![]
        ),
        Value::Long(11)
    );
    assert_eq!(
        run(
            "Function F()\n F = Mid(\"abcdef\", 2, 3)\nEnd Function",
            "F",
            vec![]
        ),
        Value::Str("bcd".into())
    );
    assert_eq!(
        run(
            "Function F()\n F = InStr(\"abcabc\", \"c\")\nEnd Function",
            "F",
            vec![]
        ),
        Value::Long(3)
    );
}

#[test]
fn on_error_resume_next_traps() {
    let src =
        "Function F()\n Dim x\n On Error Resume Next\n x = 1 / 0\n x = 42\n F = x\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(42));
}

#[test]
fn on_error_goto_handler() {
    let src = "Function F()\n On Error GoTo oops\n F = 1 / 0\n Exit Function\noops:\n F = -1\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(-1));
}

#[test]
fn err_object_number() {
    // Phase 5 wired the built-in `Err` object: after a trapped division by zero
    // (error 11), `Err.Number` reads back the code.
    let src =
        "Function F()\n On Error Resume Next\n Dim x\n x = 1 / 0\n F = Err.Number\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(11));
}

#[test]
fn err_clear_resets_number() {
    let src = "Function F()\n On Error Resume Next\n Dim x\n x = 1 / 0\n Err.Clear\n \
               F = Err.Number\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(0));
}

#[test]
fn err_raise_is_trappable() {
    let src = "Function F()\n On Error Resume Next\n Err.Raise 457\n F = Err.Number\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(457));
}

#[test]
fn division_by_zero_untrapped_errors() {
    let e = run_err("Function F()\n F = 1 / 0\nEnd Function", "F");
    assert!(matches!(e, loki_basic::BasicError::Runtime(re) if re.number == 11));
}

#[test]
fn overflow_is_reported() {
    let e = run_err(
        "Function F()\n Dim x As Integer\n x = 30000\n F = x + x\nEnd Function",
        "F",
    );
    assert!(matches!(e, loki_basic::BasicError::Runtime(re) if re.number == 6));
}

#[test]
fn fuel_exhaustion_stops_infinite_loop() {
    let src = "Function F()\n Do\n Loop\nEnd Function";
    let module = Parser::parse_module(src, Dialect::Vba).unwrap();
    let mut interp = Interp::new(&module, FuelBudget::new(10_000)).unwrap();
    let e = interp.call("F", vec![]).expect_err("should exhaust fuel");
    assert!(matches!(e, loki_basic::BasicError::Runtime(re) if re.number == 1005));
}

#[test]
fn feature_refused_for_declared_ffi() {
    // Calling a Declare'd FFI function is refused (untrappable). Wired in Phase
    // 13's builtin/refusal pass; here we only assert parsing accepts it.
    let src = "Declare Function GetTickCount Lib \"kernel32\" () As Long\nFunction F()\n F = 1\nEnd Function";
    assert_eq!(run(src, "F", vec![]), Value::Int(1));
}
