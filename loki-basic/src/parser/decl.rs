// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Declaration parsing: top-level items (procedures, `Type`, `Enum`, module
//! `Const`/`Dim`, `Declare`), plus the shared building blocks (type
//! references, variable/const declarations, parameter lists) reused by the
//! statement parser for local `Dim`/`Const`.

use super::Parser;
use crate::ast::{
    ArrayBound, ConstDecl, EnumDef, Item, Param, ProcKind, Procedure, TypeDef, TypeRef, VarDecl,
    Visibility,
};
use crate::error::BasicError;
use crate::lexer::TokenKind;

/// Parses one top-level item, or `None` if the line yielded none.
pub(super) fn parse_item(p: &mut Parser) -> Result<Option<Item>, BasicError> {
    let mut visibility = Visibility::Public;
    // Optional leading visibility / linkage modifiers.
    loop {
        if p.eat_kw("Public") || p.eat_kw("Global") || p.eat_kw("Friend") {
            visibility = Visibility::Public;
        } else if p.eat_kw("Private") {
            visibility = Visibility::Private;
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

// ── Shared building blocks (also used by statement-level Dim/Const) ─────────

impl Parser {
    /// Parses an optional `As Type` clause, returning [`TypeRef::Implicit`] when
    /// absent.
    pub(super) fn parse_as_type(&mut self) -> Result<TypeRef, BasicError> {
        if !self.eat_kw("As") {
            return Ok(TypeRef::Implicit);
        }
        let _ = self.eat_kw("New"); // `As New Class` — construction handled later.
        let name = self.expect_ident("a type name after `As`")?;
        if name.eq_ignore_ascii_case("String") && self.eat(&TokenKind::Star) {
            if let TokenKind::Int(n) = *self.peek_kind() {
                self.bump();
                let len = usize::try_from(n).unwrap_or(0);
                return Ok(TypeRef::FixedString(len));
            }
            return Err(self.error("expected a length after `String *`"));
        }
        Ok(TypeRef::Named(name))
    }

    /// Parses a comma-separated list of variable declarations (after `Dim`).
    pub(super) fn parse_var_decls(&mut self) -> Result<Vec<VarDecl>, BasicError> {
        let mut decls = vec![self.parse_one_var_decl()?];
        while self.eat(&TokenKind::Comma) {
            decls.push(self.parse_one_var_decl()?);
        }
        Ok(decls)
    }

    /// Parses one `name[(bounds)] [As Type]` variable declaration.
    pub(super) fn parse_one_var_decl(&mut self) -> Result<VarDecl, BasicError> {
        let name = self.expect_ident("a variable name")?;
        let bounds = if self.eat(&TokenKind::LParen) {
            if self.eat(&TokenKind::RParen) {
                Some(Vec::new()) // dynamic array `a()`
            } else {
                let b = self.parse_array_bounds()?;
                self.expect(&TokenKind::RParen, "`)` after array bounds")?;
                Some(b)
            }
        } else {
            None
        };
        let ty = self.parse_as_type()?;
        Ok(VarDecl { name, ty, bounds })
    }

    fn parse_array_bounds(&mut self) -> Result<Vec<ArrayBound>, BasicError> {
        let mut bounds = vec![self.parse_one_bound()?];
        while self.eat(&TokenKind::Comma) {
            bounds.push(self.parse_one_bound()?);
        }
        Ok(bounds)
    }

    fn parse_one_bound(&mut self) -> Result<ArrayBound, BasicError> {
        let first = self.parse_expr()?;
        if self.eat_kw("To") {
            let upper = self.parse_expr()?;
            Ok(ArrayBound {
                lower: Some(first),
                upper,
            })
        } else {
            Ok(ArrayBound {
                lower: None,
                upper: first,
            })
        }
    }

    /// Parses a comma-separated list of `name [As Type] = value` constants.
    pub(super) fn parse_const_decls(&mut self) -> Result<Vec<ConstDecl>, BasicError> {
        let mut decls = vec![self.parse_one_const()?];
        while self.eat(&TokenKind::Comma) {
            decls.push(self.parse_one_const()?);
        }
        Ok(decls)
    }

    fn parse_one_const(&mut self) -> Result<ConstDecl, BasicError> {
        let name = self.expect_ident("a constant name")?;
        let ty = self.parse_as_type()?;
        self.expect(&TokenKind::Eq, "`=` in a Const declaration")?;
        let value = self.parse_expr()?;
        Ok(ConstDecl { name, ty, value })
    }

    /// Consumes an `End <kw>` block terminator.
    pub(super) fn expect_end(&mut self, kw: &str) -> Result<(), BasicError> {
        if !self.eat_kw("End") {
            return Err(self.error(&format!("expected `End {kw}`")));
        }
        if !self.eat_kw(kw) {
            return Err(self.error(&format!("expected `{kw}` after `End`")));
        }
        self.end_of_statement()
    }
}
