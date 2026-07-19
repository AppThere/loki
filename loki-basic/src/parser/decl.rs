// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Declaration parsing: top-level items (procedures, `Type`, `Enum`, module
//! `Const`/`Dim`, `Declare`), plus the shared building blocks (type
//! references, variable/const declarations, parameter lists) reused by the
//! statement parser for local `Dim`/`Const`.

use super::Parser;
use crate::ast::{
    ClassDef, EnumDef, Item, Param, ProcKind, Procedure, TypeDef, VarDecl, Visibility,
};
use crate::error::BasicError;
use crate::lexer::TokenKind;

/// Parses one top-level item, or `None` if the line yielded none.
pub(super) fn parse_item(p: &mut Parser) -> Result<Option<Item>, BasicError> {
    let mut visibility = Visibility::Public;
    let mut saw_modifier = false;
    // Optional leading visibility / linkage modifiers.
    loop {
        if p.eat_kw("Public") || p.eat_kw("Global") || p.eat_kw("Friend") {
            visibility = Visibility::Public;
            saw_modifier = true;
        } else if p.eat_kw("Private") {
            visibility = Visibility::Private;
            saw_modifier = true;
        } else {
            break;
        }
    }
    let is_static = p.eat_kw("Static");

    if p.peek_kw("Sub") {
        return Ok(Some(Item::Procedure(parse_proc(
            p,
            ProcKind::Sub,
            visibility,
            is_static,
        )?)));
    }
    if p.peek_kw("Function") {
        return Ok(Some(Item::Procedure(parse_proc(
            p,
            ProcKind::Function,
            visibility,
            is_static,
        )?)));
    }
    if p.eat_kw("Property") {
        let kind = if p.eat_kw("Get") {
            ProcKind::PropertyGet
        } else if p.eat_kw("Let") {
            ProcKind::PropertyLet
        } else if p.eat_kw("Set") {
            ProcKind::PropertySet
        } else {
            return Err(p.error("expected Get, Let, or Set after Property"));
        };
        return Ok(Some(Item::Procedure(parse_proc_after_kw(
            p, kind, visibility, is_static,
        )?)));
    }
    if p.eat_kw("Class") {
        return Ok(Some(Item::Class(parse_class_def(p)?)));
    }
    if p.eat_kw("Type") {
        return Ok(Some(Item::Type(parse_type_def(p)?)));
    }
    if p.eat_kw("Enum") {
        return Ok(Some(Item::Enum(parse_enum_def(p)?)));
    }
    if p.eat_kw("Const") {
        let decls = p.parse_const_decls()?;
        p.end_of_statement()?;
        return Ok(Some(Item::Const(decls)));
    }
    if p.eat_kw("Declare") {
        return Ok(Some(parse_foreign_decl(p)?));
    }
    // Module-level variable declaration.
    if p.eat_kw("Dim") || p.eat_kw("WithEvents") {
        let decls = p.parse_var_decls()?;
        p.end_of_statement()?;
        return Ok(Some(Item::Var(decls)));
    }
    // A bare `Public`/`Private`/`Static name As Type` variable (no `Dim`) — the
    // usual form for module-level and class-module fields.
    if (saw_modifier || is_static) && matches!(p.peek_kind(), TokenKind::Ident(_)) {
        let decls = p.parse_var_decls()?;
        p.end_of_statement()?;
        return Ok(Some(Item::Var(decls)));
    }
    Err(p.error("expected a declaration (Sub, Function, Dim, Const, Type, Enum, …)"))
}

fn parse_proc(
    p: &mut Parser,
    kind: ProcKind,
    visibility: Visibility,
    is_static: bool,
) -> Result<Procedure, BasicError> {
    p.bump(); // Sub / Function
    parse_proc_after_kw(p, kind, visibility, is_static)
}

fn parse_proc_after_kw(
    p: &mut Parser,
    kind: ProcKind,
    visibility: Visibility,
    is_static: bool,
) -> Result<Procedure, BasicError> {
    let name = p.expect_ident("a procedure name")?;
    let params = if p.eat(&TokenKind::LParen) {
        let ps = parse_params(p)?;
        p.expect(&TokenKind::RParen, "`)` to close the parameter list")?;
        ps
    } else {
        Vec::new()
    };
    let ret_ty = p.parse_as_type()?;
    p.end_of_statement()?;

    let end_kw = match kind {
        ProcKind::Sub => "Sub",
        ProcKind::Function => "Function",
        _ => "Property",
    };
    let body = p.parse_block(&|q| q.peek_kw("End"))?;
    p.expect_end(end_kw)?;
    Ok(Procedure {
        name,
        kind,
        visibility,
        is_static,
        params,
        ret_ty,
        body,
    })
}

fn parse_params(p: &mut Parser) -> Result<Vec<Param>, BasicError> {
    let mut params = Vec::new();
    if matches!(p.peek_kind(), TokenKind::RParen) {
        return Ok(params);
    }
    loop {
        params.push(parse_one_param(p)?);
        if !p.eat(&TokenKind::Comma) {
            break;
        }
    }
    Ok(params)
}

fn parse_one_param(p: &mut Parser) -> Result<Param, BasicError> {
    let optional = p.eat_kw("Optional");
    let mut by_val = false;
    if p.eat_kw("ByVal") {
        by_val = true;
    } else {
        let _ = p.eat_kw("ByRef");
    }
    let param_array = p.eat_kw("ParamArray");
    let name = p.expect_ident("a parameter name")?;
    let is_array = if p.eat(&TokenKind::LParen) {
        p.expect(&TokenKind::RParen, "`)` after array parameter `()`")?;
        true
    } else {
        false
    };
    let ty = p.parse_as_type()?;
    let default = if optional && p.eat(&TokenKind::Eq) {
        Some(p.parse_expr()?)
    } else {
        None
    };
    Ok(Param {
        name,
        by_val,
        optional,
        param_array,
        is_array,
        ty,
        default,
    })
}

fn parse_type_def(p: &mut Parser) -> Result<TypeDef, BasicError> {
    let name = p.expect_ident("a type name")?;
    p.end_of_statement()?;
    let mut fields = Vec::new();
    p.skip_terminators();
    while !p.at_eof() && !p.peek_kw("End") {
        fields.push(p.parse_one_var_decl()?);
        p.end_of_statement()?;
        p.skip_terminators();
    }
    p.expect_end("Type")?;
    Ok(TypeDef { name, fields })
}

/// `Class <Name> … End Class` — a class module (macro spec §4.2, phase 6). The
/// body is the same items a module accepts, sorted into instance fields
/// (`Dim`/`Private`/`Public`) and methods (`Sub`/`Function`/`Property`). Nested
/// classes/types/enums/`Declare`s are rejected (a class body is flat).
fn parse_class_def(p: &mut Parser) -> Result<ClassDef, BasicError> {
    let name = p.expect_ident("a class name")?;
    p.end_of_statement()?;
    let mut fields: Vec<VarDecl> = Vec::new();
    let mut methods: Vec<Procedure> = Vec::new();
    p.skip_terminators();
    while !p.at_eof() && !p.peek_kw("End") {
        match parse_item(p)? {
            Some(Item::Procedure(proc)) => methods.push(proc),
            Some(Item::Var(decls)) => fields.extend(decls),
            _ => return Err(p.error("a class body allows only fields and methods")),
        }
        p.skip_terminators();
    }
    p.expect_end("Class")?;
    Ok(ClassDef {
        name,
        fields,
        methods,
    })
}

fn parse_enum_def(p: &mut Parser) -> Result<EnumDef, BasicError> {
    let name = p.expect_ident("an enum name")?;
    p.end_of_statement()?;
    let mut members = Vec::new();
    p.skip_terminators();
    while !p.at_eof() && !p.peek_kw("End") {
        let m = p.expect_ident("an enum member name")?;
        let value = if p.eat(&TokenKind::Eq) {
            Some(p.parse_expr()?)
        } else {
            None
        };
        members.push((m, value));
        p.end_of_statement()?;
        p.skip_terminators();
    }
    p.expect_end("Enum")?;
    Ok(EnumDef { name, members })
}

/// `Declare [PtrSafe] Function|Sub name Lib "…" …` — an FFI declaration. We
/// capture only the name; calling it is refused at runtime (spec §7).
fn parse_foreign_decl(p: &mut Parser) -> Result<Item, BasicError> {
    let _ = p.eat_kw("PtrSafe");
    let _ = p.eat_kw("Function") || p.eat_kw("Sub");
    let name = p.expect_ident("a foreign procedure name")?;
    // Skip the remainder of the declaration line (Lib "…" [Alias "…"] params).
    while !p.at_stmt_end() {
        p.bump();
    }
    p.end_of_statement()?;
    Ok(Item::ForeignDecl { name })
}
