// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! User-defined class-module instances (macro spec §4.2, phase 6).
//!
//! A class instance is **pure interpreter heap** — a bag of fields plus method
//! dispatch on the receiver (`Me`). It is represented as an ordinary
//! [`Value::Object`] handle so `Set`, `Is`, and `With` keep working unchanged,
//! but its handle is allocated from [`USER_OBJ_BASE`] upward and its state lives
//! in the interpreter's own `instances` table. Dispatch checks that table
//! **before** the [`crate::Host`] fallthrough, so a class instance never reaches
//! the capability seam: a user class can grant a script no authority it did not
//! already have.

use std::collections::HashMap;

use super::Interp;
use super::env::key;
use crate::ast::{ClassDef, ProcKind, Procedure};
use crate::error::{BasicError, RuntimeError};
use crate::host::{Host, ObjectRef};
use crate::value::Value;

/// The first user-instance handle. Chosen well above the small handles a
/// [`Host`] hands out for its own objects (`Application`, `ActiveDocument`, …),
/// so the interpreter's `instances`-table check never shadows a host object.
pub(super) const USER_OBJ_BASE: u32 = 0x4000_0000;

/// One live class-module instance: its class (lowercased key) and field values.
pub(super) struct Instance {
    class: String,
    fields: HashMap<String, Value>,
}

impl<'m, H: Host> Interp<'m, H> {
    /// Constructs a `New <class>` instance: allocates a handle, default-initialises
    /// its declared fields, and returns the object value. An unknown class name is
    /// **not** a user class — it is an external ProgID/COM object, refused (§7).
    pub(super) fn construct_instance(&mut self, class_name: &str) -> Result<Value, RuntimeError> {
        let Some(&def) = self.classes.get(&key(class_name)) else {
            return Err(RuntimeError::feature_refused(&format!("New {class_name}")));
        };
        let mut fields = HashMap::new();
        for decl in &def.fields {
            let v = self.default_value(decl).map_err(demote)?;
            fields.insert(key(&decl.name), v);
        }
        let id = self.next_obj;
        self.next_obj = self.next_obj.wrapping_add(1);
        self.instances.insert(
            id,
            Instance {
                class: key(&def.name),
                fields,
            },
        );
        Ok(Value::Object(ObjectRef(id)))
    }

    /// Whether `r` is a user class instance (vs. a host object).
    pub(super) fn is_instance(&self, r: ObjectRef) -> bool {
        r.0 >= USER_OBJ_BASE && self.instances.contains_key(&r.0)
    }

    /// The class definition backing an instance handle, if any.
    fn class_of(&self, r: ObjectRef) -> Option<&'m ClassDef> {
        let inst = self.instances.get(&r.0)?;
        self.classes.get(&inst.class).copied()
    }

    fn has_field(&self, r: ObjectRef, name: &str) -> bool {
        self.instances
            .get(&r.0)
            .is_some_and(|i| i.fields.contains_key(&key(name)))
    }

    fn read_field(&self, r: ObjectRef, name: &str) -> Value {
        self.instances
            .get(&r.0)
            .and_then(|i| i.fields.get(&key(name)))
            .cloned()
            .unwrap_or(Value::Empty)
    }

    fn write_field(&mut self, r: ObjectRef, name: &str, value: Value) {
        if let Some(i) = self.instances.get_mut(&r.0) {
            i.fields.insert(key(name), value);
        }
    }

    /// Whether the instance's class exposes a **callable** member of `name`
    /// (`Sub`/`Function`/`Property Get`) — used to decide bare-name dispatch.
    pub(super) fn instance_has_method(&self, r: ObjectRef, name: &str) -> bool {
        self.class_of(r)
            .and_then(|c| method_by(c, name, ProcKind::returns_value_or_sub))
            .is_some()
    }

    /// Property read `obj.name`: `Property Get` → field → zero-arg method → 438.
    pub(super) fn instance_get(&mut self, r: ObjectRef, name: &str) -> Result<Value, RuntimeError> {
        let class = self.class_of(r).ok_or_else(no_member)?;
        if let Some(p) = method_by(class, name, |k| k == ProcKind::PropertyGet) {
            return self.invoke_method(p, r, Vec::new());
        }
        if self.has_field(r, name) {
            return Ok(self.read_field(r, name));
        }
        if let Some(p) = method_by(class, name, ProcKind::returns_value_or_sub) {
            return self.invoke_method(p, r, Vec::new());
        }
        Err(no_member())
    }

    /// Method call `obj.name(args)`: a `Sub`/`Function`/`Property Get` member.
    pub(super) fn instance_call(
        &mut self,
        r: ObjectRef,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let class = self.class_of(r).ok_or_else(no_member)?;
        match method_by(class, name, ProcKind::returns_value_or_sub) {
            Some(p) => self.invoke_method(p, r, args),
            None => Err(no_member()),
        }
    }

    /// Property assignment `obj.name = value`: `Property Let`/`Set` → field → 438.
    pub(super) fn instance_set(
        &mut self,
        r: ObjectRef,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let class = self.class_of(r).ok_or_else(no_member)?;
        if let Some(p) = method_by(class, name, |k| {
            matches!(k, ProcKind::PropertyLet | ProcKind::PropertySet)
        }) {
            self.invoke_method(p, r, vec![value])?;
            return Ok(());
        }
        if self.has_field(r, name) {
            self.write_field(r, name, value);
            return Ok(());
        }
        Err(no_member())
    }

    /// Bare-name read inside a method body: resolves against `Me`'s members, or
    /// `None` to fall through to module-level resolution.
    pub(super) fn instance_implicit_get(
        &mut self,
        me: ObjectRef,
        name: &str,
    ) -> Result<Option<Value>, RuntimeError> {
        let Some(class) = self.class_of(me) else {
            return Ok(None);
        };
        if let Some(p) = method_by(class, name, |k| k == ProcKind::PropertyGet) {
            return self.invoke_method(p, me, Vec::new()).map(Some);
        }
        if self.has_field(me, name) {
            return Ok(Some(self.read_field(me, name)));
        }
        if let Some(p) = method_by(class, name, ProcKind::returns_value_or_sub) {
            return self.invoke_method(p, me, Vec::new()).map(Some);
        }
        Ok(None)
    }

    /// Whether `Me` has a settable member `name` (field or `Property Let`/`Set`).
    pub(super) fn instance_has_settable(&self, me: ObjectRef, name: &str) -> bool {
        if self.has_field(me, name) {
            return true;
        }
        self.class_of(me)
            .and_then(|c| {
                method_by(c, name, |k| {
                    matches!(k, ProcKind::PropertyLet | ProcKind::PropertySet)
                })
            })
            .is_some()
    }

    /// Invokes a class method with the receiver bound as `Me`. Arguments bind
    /// positionally (`ParamArray` supported); there is no `ByRef` copy-out for
    /// method calls in v1.
    fn invoke_method(
        &mut self,
        proc: &'m Procedure,
        me: ObjectRef,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let mut callee = self.new_frame(proc);
        callee.me = Some(me);
        let mut it = args.into_iter();
        for param in &proc.params {
            if param.param_array {
                let rest: Vec<Value> = it.by_ref().collect();
                callee.set(&param.name, super::call::variant_array(rest)?);
                break;
            }
            callee.set(&param.name, it.next().unwrap_or(Value::Empty));
        }
        self.run_proc(proc, &mut callee)
    }
}

impl ProcKind {
    /// A value-returning member or a plain `Sub` — the kinds `obj.Member(...)`
    /// and bare-name dispatch treat as callable methods.
    fn returns_value_or_sub(self) -> bool {
        matches!(
            self,
            ProcKind::Sub | ProcKind::Function | ProcKind::PropertyGet
        )
    }
}

/// Finds a method of `class` named `name` (case-insensitive) whose kind matches.
fn method_by<'a>(
    class: &'a ClassDef,
    name: &str,
    pred: impl Fn(ProcKind) -> bool,
) -> Option<&'a Procedure> {
    class
        .methods
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(name) && pred(m.kind))
}

/// Error 438 — object doesn't support this property or method.
fn no_member() -> RuntimeError {
    RuntimeError::new(438, "Object doesn't support this property or method")
}

/// Demotes a construction-time [`BasicError`] to a [`RuntimeError`] (only array
/// bounds on a field can fail; a non-runtime error maps to "invalid call").
fn demote(e: BasicError) -> RuntimeError {
    match e {
        BasicError::Runtime(r) => r,
        _ => RuntimeError::new(5, "Invalid procedure call or argument"),
    }
}
