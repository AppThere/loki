// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parser integration tests: real BASIC snippets → AST shape checks.

use loki_basic::Dialect;
use loki_basic::ast::{BinOp, Expr, Item, ProcKind, Stmt};
use loki_basic::parser::Parser;

fn parse(src: &str) -> loki_basic::ast::Module {
    Parser::parse_module(src, Dialect::Vba).expect("parse")
}

fn first_proc_body(src: &str) -> Vec<Stmt> {
    let m = parse(src);
    match m.items.into_iter().next() {
        Some(Item::Procedure(p)) => p.body,
        _ => panic!("expected a procedure"),
    }
}

#[test]
fn empty_sub() {
    let m = parse("Sub Main\nEnd Sub");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        Item::Procedure(p) => {
            assert_eq!(p.name, "Main");
            assert_eq!(p.kind, ProcKind::Sub);
            assert!(p.body.is_empty());
        }
        _ => panic!(),
    }
}

#[test]
fn function_with_params_and_return_type() {
    let m = parse("Function Add(a As Long, b As Long) As Long\n  Add = a + b\nEnd Function");
    match &m.items[0] {
        Item::Procedure(p) => {
            assert_eq!(p.kind, ProcKind::Function);
            assert_eq!(p.params.len(), 2);
            assert_eq!(p.params[0].name, "a");
            assert_eq!(p.body.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn precedence_pow_binds_tighter_than_unary_minus() {
    // -2 ^ 2  ==  -(2 ^ 2)
    let body = first_proc_body("Sub S\n x = -2 ^ 2\nEnd Sub");
    let Stmt::Assign { value, .. } = &body[0] else {
        panic!("expected assign");
    };
    match value {
        Expr::Unary { op, operand } => {
            assert!(matches!(op, loki_basic::ast::UnOp::Neg));
            assert!(matches!(**operand, Expr::Binary { op: BinOp::Pow, .. }));
        }
        other => panic!("expected unary neg over pow, got {other:?}"),
    }
}

#[test]
fn concat_binds_tighter_than_comparison() {
    // "a" & "b" = "ab"  parses as  ("a" & "b") = "ab"
    let body = first_proc_body(r#"Sub S: x = "a" & "b" = "ab": End Sub"#);
    let Stmt::Assign { value, .. } = &body[0] else {
        panic!();
    };
    assert!(matches!(value, Expr::Binary { op: BinOp::Eq, .. }));
}

#[test]
fn block_if_elseif_else() {
    let body = first_proc_body(
        "Sub S\n If a Then\n  x = 1\n ElseIf b Then\n  x = 2\n Else\n  x = 3\n End If\nEnd Sub",
    );
    let Stmt::If {
        branches,
        else_body,
    } = &body[0]
    else {
        panic!("expected if");
    };
    assert_eq!(branches.len(), 2);
    assert!(else_body.is_some());
}

#[test]
fn single_line_if() {
    let body = first_proc_body("Sub S\n If a Then x = 1 Else x = 2\nEnd Sub");
    let Stmt::If {
        branches,
        else_body,
    } = &body[0]
    else {
        panic!();
    };
    assert_eq!(branches[0].1.len(), 1);
    assert!(else_body.is_some());
}

#[test]
fn for_loop_with_step() {
    let body =
        first_proc_body("Sub S\n For i = 1 To 10 Step 2\n  total = total + i\n Next i\nEnd Sub");
    assert!(matches!(body[0], Stmt::For { .. }));
}

#[test]
fn do_until_loop() {
    let body = first_proc_body("Sub S\n Do Until done\n  x = x + 1\n Loop\nEnd Sub");
    assert!(matches!(body[0], Stmt::DoLoop { .. }));
}

#[test]
fn select_case_with_ranges_and_is() {
    let src = "Sub S\n Select Case n\n Case 1, 2\n  x = 1\n Case 3 To 5\n  x = 2\n Case Is > 10\n  x = 3\n Case Else\n  x = 4\n End Select\nEnd Sub";
    let body = first_proc_body(src);
    let Stmt::SelectCase {
        cases, else_body, ..
    } = &body[0]
    else {
        panic!("expected select");
    };
    assert_eq!(cases.len(), 3);
    assert!(else_body.is_some());
}

#[test]
fn bare_call_with_args() {
    // MsgBox "hi", 0  →  a call statement with two args
    let body = first_proc_body(r#"Sub S: MsgBox "hi", 0: End Sub"#);
    let Stmt::Call(Expr::Call { args, .. }) = &body[0] else {
        panic!("expected bare call, got {:?}", body[0]);
    };
    assert_eq!(args.len(), 2);
}

#[test]
fn named_arguments() {
    let body = first_proc_body(r#"Sub S: f x:=1, y:=2: End Sub"#);
    let Stmt::Call(Expr::Call { args, .. }) = &body[0] else {
        panic!("expected call");
    };
    assert_eq!(args[0].name.as_deref(), Some("x"));
    assert_eq!(args[1].name.as_deref(), Some("y"));
}

#[test]
fn dim_array_with_bounds() {
    let body = first_proc_body("Sub S\n Dim a(1 To 10) As Long\nEnd Sub");
    let Stmt::Dim(decls) = &body[0] else {
        panic!();
    };
    assert!(decls[0].bounds.is_some());
}

#[test]
fn on_error_and_labels() {
    let body = first_proc_body(
        "Sub S\n On Error GoTo handler\n x = 1\nhandler:\n y = 2\n On Error GoTo 0\nEnd Sub",
    );
    assert!(matches!(body[0], Stmt::OnError(_)));
    assert!(
        body.iter()
            .any(|s| matches!(s, Stmt::Label(l) if l == "handler"))
    );
}

#[test]
fn foreign_declare_is_captured_not_rejected() {
    let m = parse("Declare Function GetTickCount Lib \"kernel32\" () As Long");
    assert!(matches!(&m.items[0], Item::ForeignDecl { name } if name == "GetTickCount"));
}

#[test]
fn type_and_enum_declarations() {
    let m = parse(
        "Type Point\n x As Long\n y As Long\nEnd Type\nEnum Color\n Red\n Green = 5\n Blue\nEnd Enum",
    );
    assert!(matches!(&m.items[0], Item::Type(t) if t.fields.len() == 2));
    assert!(matches!(&m.items[1], Item::Enum(e) if e.members.len() == 3));
}

#[test]
fn option_base_and_explicit() {
    let m = parse("Option Explicit\nOption Base 1\nSub S\nEnd Sub");
    assert!(m.options.explicit);
    assert_eq!(m.options.base, 1);
}

#[test]
fn with_block_and_leading_dot() {
    let body = first_proc_body("Sub S\n With obj\n  .Name = \"x\"\n End With\nEnd Sub");
    let Stmt::With { body: inner, .. } = &body[0] else {
        panic!();
    };
    let Stmt::Assign { target, .. } = &inner[0] else {
        panic!();
    };
    assert!(matches!(
        target,
        Expr::Member { object, .. } if matches!(**object, Expr::WithContext)
    ));
}
