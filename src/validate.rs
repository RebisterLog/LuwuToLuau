use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::config::FailOnFlags;

/// A validation diagnostic.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub category: &'static str, // "syntax" | "contracts" | "types"
    pub message: String,
}

impl Diagnostic {
    pub fn contracts(message: impl Into<String>) -> Self {
        Self {
            category: "contracts",
            message: message.into(),
        }
    }

    pub fn types(message: impl Into<String>) -> Self {
        Self {
            category: "types",
            message: message.into(),
        }
    }

    pub fn blocks(&self, flags: &FailOnFlags) -> bool {
        match self.category {
            "syntax" => flags.syntax,
            "contracts" => flags.contracts,
            "types" => flags.types,
            _ => true,
        }
    }
}

/// Validates a parsed program and returns diagnostics for enabled/known
/// categories. Callers decide which ones block the build based on `fail_on`.
pub fn validate_program(program: &Program) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    validate_contracts(program, &mut out);
    validate_types(program, &mut out);
    out
}

#[derive(Clone, Default)]
#[allow(dead_code)]
struct TraitInfo {
    members: HashMap<String, TraitMember>,
    params: Vec<String>,
    needs: Vec<String>,
}

#[derive(Clone)]
#[allow(dead_code)]
enum TraitMember {
    ExpectMethod {
        static_method: bool,
        param_count: usize,
        return_type: Option<String>,
    },
    ExpectField {
        ty: Option<String>,
    },
    Method {
        is_final: bool,
        static_method: bool,
        param_count: usize,
        return_type: Option<String>,
    },
    Field {
        ty: Option<String>,
    },
}

fn collect_traits(program: &Program) -> HashMap<String, TraitInfo> {
    let mut traits = HashMap::new();
    for stmt in &program.stmts {
        if let Stmt::Trait(t) = stmt {
            let mut info = TraitInfo {
                params: t.params.iter().map(|p| p.name.clone()).collect(),
                needs: t.needs.iter().map(|n| n.name.clone()).collect(),
                ..TraitInfo::default()
            };
            for m in &t.members {
                match m {
                    ClassMember::Field(f) => {
                        if f.is_expect {
                            info.members.insert(
                                f.name.clone(),
                                TraitMember::ExpectField { ty: f.ty.clone() },
                            );
                        } else {
                            info.members
                                .insert(f.name.clone(), TraitMember::Field { ty: f.ty.clone() });
                        }
                    }
                    ClassMember::Method(m) => {
                        let static_method =
                            m.is_static || !m.params.first().is_some_and(|p| p.name == "self");
                        let param_count = if !static_method
                            && m.params.first().is_some_and(|p| p.name == "self")
                        {
                            m.params.len().saturating_sub(1)
                        } else {
                            m.params.len()
                        };
                        if m.is_expect {
                            info.members.insert(
                                m.name.clone(),
                                TraitMember::ExpectMethod {
                                    static_method,
                                    param_count,
                                    return_type: m.return_type.clone(),
                                },
                            );
                        } else {
                            info.members.insert(
                                m.name.clone(),
                                TraitMember::Method {
                                    is_final: m.is_final,
                                    static_method,
                                    param_count,
                                    return_type: m.return_type.clone(),
                                },
                            );
                        }
                    }
                }
            }
            traits.insert(t.name.clone(), info);
        }
    }
    traits
}

fn resolve_trait_members(
    name: &str,
    traits: &HashMap<String, TraitInfo>,
    out: &mut HashMap<String, TraitMember>,
    seen: &mut HashSet<String>,
) {
    if !seen.insert(name.to_string()) {
        return;
    }
    if let Some(info) = traits.get(name) {
        for need in &info.needs {
            resolve_trait_members(need, traits, out, seen);
        }
        for (k, v) in &info.members {
            out.insert(k.clone(), v.clone());
        }
    }
}

fn validate_contracts(program: &Program, out: &mut Vec<Diagnostic>) {
    let traits = collect_traits(program);

    // Unknown traits in `needs` are only reported when the trait definition
    // is local but the referenced trait is not present in this file. In
    // multi-file projects the definition may live elsewhere, so this is best
    // treated as a non-fatal limitation of per-file validation.
    // (No diagnostic emitted here by design.)

    for stmt in &program.stmts {
        if let Stmt::Class(c) = stmt {
            // Gather class members.
            let mut class_fields: HashMap<String, &FieldDecl> = HashMap::new();
            let mut class_methods: HashMap<String, &MethodDecl> = HashMap::new();
            let mut ctor_params: HashMap<String, &Param> = HashMap::new();
            for p in &c.params {
                ctor_params.insert(p.name.clone(), p);
            }
            for m in &c.members {
                match m {
                    ClassMember::Field(f) => {
                        class_fields.insert(f.name.clone(), f);
                    }
                    ClassMember::Method(m) => {
                        class_methods.insert(m.name.clone(), m);
                    }
                }
            }

            for imp in &c.implements {
                if !traits.contains_key(&imp.name) {
                    // Trait may be defined in another file; per-file validation
                    // cannot verify external contracts.
                    continue;
                }

                let mut all_members: HashMap<String, TraitMember> = HashMap::new();
                let mut seen = HashSet::new();
                resolve_trait_members(&imp.name, &traits, &mut all_members, &mut seen);

                for (name, member) in &all_members {
                    match member {
                        TraitMember::ExpectField { ty } => {
                            let provided = class_fields
                                .get(name)
                                .map(|f| !f.is_expect)
                                .unwrap_or(false)
                                || ctor_params.contains_key(name)
                                || class_methods.contains_key(name)
                                || has_provided_by_trait(name, &all_members);
                            if !provided {
                                out.push(Diagnostic::contracts(format!(
                                    "class `{}` does not implement expected member `{}` required by trait `{}`",
                                    c.name, name, imp.name
                                )));
                            } else if let Some(cf) = class_fields.get(name) {
                                if !cf.is_expect {
                                    check_type_compat_field(
                                        name,
                                        ty.as_deref(),
                                        cf.ty.as_deref(),
                                        &c.name,
                                        &imp.name,
                                        out,
                                    );
                                }
                            } else if let Some(cp) = ctor_params.get(name) {
                                check_type_compat_field(
                                    name,
                                    ty.as_deref(),
                                    cp.ty.as_deref(),
                                    &c.name,
                                    &imp.name,
                                    out,
                                );
                            }
                        }
                        TraitMember::ExpectMethod {
                            static_method,
                            param_count,
                            return_type,
                        } => {
                            if let Some(cm) = class_methods.get(name) {
                                if cm.is_expect {
                                    out.push(Diagnostic::contracts(format!(
                                        "class `{}` does not implement expected method `{}` required by trait `{}`",
                                        c.name, name, imp.name
                                    )));
                                    continue;
                                }
                                let cm_static = cm.is_static
                                    || !cm.params.first().is_some_and(|p| p.name == "self");
                                if cm_static != *static_method {
                                    out.push(Diagnostic::contracts(format!(
                                        "class `{}` implements method `{}` with wrong kind (expected {})",
                                        c.name,
                                        name,
                                        if *static_method { "static" } else { "instance" }
                                    )));
                                }
                                let cm_count = if !cm_static
                                    && cm.params.first().is_some_and(|p| p.name == "self")
                                {
                                    cm.params.len().saturating_sub(1)
                                } else {
                                    cm.params.len()
                                };
                                if cm_count != *param_count {
                                    out.push(Diagnostic::contracts(format!(
                                        "class `{}` implements method `{}` with {} parameter(s), expected {}",
                                        c.name, name, cm_count, param_count
                                    )));
                                }
                                check_return_type_compat(
                                    name,
                                    return_type.as_deref(),
                                    cm.return_type.as_deref(),
                                    &c.name,
                                    &imp.name,
                                    out,
                                );
                            } else if !has_provided_by_trait(name, &all_members) {
                                out.push(Diagnostic::contracts(format!(
                                    "class `{}` does not implement expected method `{}` required by trait `{}`",
                                    c.name, name, imp.name
                                )));
                            }
                        }
                        TraitMember::Method { is_final, .. }
                            if *is_final && class_methods.contains_key(name) =>
                        {
                            out.push(Diagnostic::contracts(format!(
                                "class `{}` overrides final method `{}` from trait `{}`",
                                c.name, name, imp.name
                            )));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn has_provided_by_trait(name: &str, members: &HashMap<String, TraitMember>) -> bool {
    matches!(
        members.get(name),
        Some(TraitMember::Method { .. } | TraitMember::Field { .. })
    )
}

fn normalize_type(ty: &str) -> String {
    ty.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn types_explicitly_incompatible(expected: &str, actual: &str) -> bool {
    let e = normalize_type(expected);
    let a = normalize_type(actual);
    if e == a || e.is_empty() || a.is_empty() {
        return false;
    }
    // Don't fight complex/generic/union types in this lightweight pass.
    if e.contains('|')
        || a.contains('|')
        || e.contains('{')
        || a.contains('{')
        || e.contains('<')
        || a.contains('<')
        || e.contains("->")
        || a.contains("->")
    {
        return false;
    }
    let simple = |t: &str| {
        matches!(
            t,
            "string"
                | "number"
                | "boolean"
                | "nil"
                | "none"
                | "any"
                | "unknown"
                | "never"
                | "thread"
                | "userdata"
                | "buffer"
        )
    };
    if e == "any" || e == "unknown" || a == "any" || a == "unknown" {
        return false;
    }
    if simple(&e) && simple(&a) {
        return e != a;
    }
    // Different named simple types are likely incompatible only when both are primitives.
    false
}

fn check_type_compat_field(
    name: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    class: &str,
    trait_name: &str,
    out: &mut Vec<Diagnostic>,
) {
    if let (Some(e), Some(a)) = (expected, actual) {
        if types_explicitly_incompatible(e, a) {
            out.push(Diagnostic::types(format!(
                "class `{}` member `{}` has type `{}` but trait `{}` expects `{}`",
                class,
                name,
                actual.unwrap_or(""),
                trait_name,
                expected.unwrap_or("")
            )));
        }
    }
}

fn check_return_type_compat(
    name: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    class: &str,
    trait_name: &str,
    out: &mut Vec<Diagnostic>,
) {
    if let (Some(e), Some(a)) = (expected, actual) {
        if types_explicitly_incompatible(e, a) {
            out.push(Diagnostic::types(format!(
                "class `{}` method `{}` returns `{}` but trait `{}` expects `{}`",
                class,
                name,
                actual.unwrap_or(""),
                trait_name,
                expected.unwrap_or("")
            )));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KnownType {
    Number,
    String,
    Boolean,
    Nil,
    None,
    Other(String),
    Unknown,
}

fn known_from_annotation(ty: &str) -> KnownType {
    let n = normalize_type(ty);
    // Strip optional marker `?` suffix for simple cases like `number?`
    let base = n.trim_end_matches('?').to_string();
    match base.as_str() {
        "number" => KnownType::Number,
        "string" => KnownType::String,
        "boolean" | "bool" => KnownType::Boolean,
        "nil" => KnownType::Nil,
        "none" => KnownType::None,
        "" => KnownType::Unknown,
        _ => KnownType::Other(base),
    }
}

fn is_optional_annotation(ty: &str) -> bool {
    let n = normalize_type(ty);
    n.ends_with('?') || n.contains("|nil") || n.contains("nil|")
}

fn literal_type(expr: &Expr) -> KnownType {
    match expr {
        Expr::Number(_) => KnownType::Number,
        Expr::Str(_) | Expr::Backtick(_) => KnownType::String,
        Expr::Bool(_) => KnownType::Boolean,
        Expr::Nil => KnownType::Nil,
        Expr::None => KnownType::None,
        Expr::Paren(e) => literal_type(e),
        _ => KnownType::Unknown,
    }
}

fn expr_known_type(expr: &Expr, ctx: &TypeCtx) -> KnownType {
    let lt = literal_type(expr);
    if !matches!(lt, KnownType::Unknown) {
        return lt;
    }
    match expr {
        Expr::Ident(name) => ctx
            .vars
            .get(name)
            .map(|(_, kt)| kt.clone())
            .unwrap_or(KnownType::Unknown),
        Expr::Paren(e) => expr_known_type(e, ctx),
        _ => KnownType::Unknown,
    }
}

fn check_assignable(
    target_ty: &KnownType,
    target_ann: Option<&str>,
    value: &KnownType,
    context: &str,
    out: &mut Vec<Diagnostic>,
) {
    if matches!(target_ty, KnownType::Unknown) || matches!(value, KnownType::Unknown) {
        return;
    }

    // Only flag clear primitive mismatches.
    let is_clear_mismatch = matches!(
        (target_ty, value),
        (KnownType::Number, KnownType::String)
            | (KnownType::Number, KnownType::Boolean)
            | (KnownType::String, KnownType::Number)
            | (KnownType::String, KnownType::Boolean)
            | (KnownType::Boolean, KnownType::Number)
            | (KnownType::Boolean, KnownType::String)
    );

    let nil_ok =
        matches!(value, KnownType::Nil) && target_ann.map(is_optional_annotation).unwrap_or(false);

    if is_clear_mismatch && !nil_ok {
        out.push(Diagnostic::types(format!(
            "{}: cannot assign {} to {}",
            context,
            type_name(value),
            target_ann.unwrap_or(type_name(target_ty))
        )));
    }
}

fn type_name(t: &KnownType) -> &'static str {
    match t {
        KnownType::Number => "number",
        KnownType::String => "string",
        KnownType::Boolean => "boolean",
        KnownType::Nil => "nil",
        KnownType::None => "none",
        KnownType::Other(_) => "value",
        KnownType::Unknown => "unknown",
    }
}

struct TypeCtx {
    vars: HashMap<String, (Option<String>, KnownType)>,
    funcs: HashMap<String, FuncSig>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct FuncSig {
    params: Vec<(Option<String>, KnownType)>,
    ret: Option<String>,
}

impl TypeCtx {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            funcs: HashMap::new(),
        }
    }

    fn child(&self) -> Self {
        Self {
            vars: self.vars.clone(),
            funcs: self.funcs.clone(),
        }
    }
}

fn validate_types(program: &Program, out: &mut Vec<Diagnostic>) {
    let mut ctx = TypeCtx::new();
    // Pre-collect top-level function signatures.
    for stmt in &program.stmts {
        if let Stmt::Function {
            name,
            params,
            return_type,
            ..
        } = stmt
        {
            ctx.funcs.insert(
                name.clone(),
                FuncSig {
                    params: params
                        .iter()
                        .map(|p| {
                            (
                                p.ty.clone(),
                                p.ty.as_deref()
                                    .map(known_from_annotation)
                                    .unwrap_or(KnownType::Unknown),
                            )
                        })
                        .collect(),
                    ret: return_type.clone(),
                },
            );
        }
    }
    validate_stmts(&program.stmts, &mut ctx, None, out);
}

fn validate_stmts(
    stmts: &[Stmt],
    ctx: &mut TypeCtx,
    current_ret: Option<&str>,
    out: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        validate_stmt(stmt, ctx, current_ret, out);
    }
}

fn validate_stmt(
    stmt: &Stmt,
    ctx: &mut TypeCtx,
    current_ret: Option<&str>,
    out: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Decl {
            patterns,
            exprs,
            ty,
            ..
        } => {
            for (i, p) in patterns.iter().enumerate() {
                if let Pattern::Name(name) = p {
                    let ann = ty.clone();
                    let kt = ann
                        .as_deref()
                        .map(known_from_annotation)
                        .unwrap_or(KnownType::Unknown);
                    if let (Some(ann_s), Some(expr)) = (&ann, exprs.get(i)) {
                        let vt = expr_known_type(expr, ctx);
                        check_assignable(
                            &kt,
                            Some(ann_s),
                            &vt,
                            &format!("declaration of `{}`", name),
                            out,
                        );
                    }
                    ctx.vars.insert(name.clone(), (ann, kt));
                }
            }
            for e in exprs {
                validate_expr(e, ctx, out);
            }
        }
        Stmt::Function {
            name,
            params,
            body,
            return_type,
            ..
        } => {
            let sig = FuncSig {
                params: params
                    .iter()
                    .map(|p| {
                        (
                            p.ty.clone(),
                            p.ty.as_deref()
                                .map(known_from_annotation)
                                .unwrap_or(KnownType::Unknown),
                        )
                    })
                    .collect(),
                ret: return_type.clone(),
            };
            ctx.funcs.insert(name.clone(), sig.clone());
            let mut fctx = ctx.child();
            for p in params {
                let kt =
                    p.ty.as_deref()
                        .map(known_from_annotation)
                        .unwrap_or(KnownType::Unknown);
                fctx.vars.insert(p.name.clone(), (p.ty.clone(), kt));
            }
            validate_stmts(body, &mut fctx, return_type.as_deref(), out);
        }
        Stmt::Assign { targets, exprs, .. } => {
            for (i, t) in targets.iter().enumerate() {
                if let Expr::Ident(name) = t {
                    if let Some((ann, kt)) = ctx.vars.get(name).cloned() {
                        if let (Some(ann_s), Some(expr)) = (ann.as_ref(), exprs.get(i)) {
                            let vt = expr_known_type(expr, ctx);
                            check_assignable(
                                &kt,
                                Some(ann_s),
                                &vt,
                                &format!("assignment to `{}`", name),
                                out,
                            );
                        }
                    }
                }
            }
            for e in targets {
                validate_expr(e, ctx, out);
            }
            for e in exprs {
                validate_expr(e, ctx, out);
            }
        }
        Stmt::Return(exprs) => {
            if let Some(ret_ann) = current_ret {
                if let Some(first) = exprs.first() {
                    let vt = expr_known_type(first, ctx);
                    let kt = known_from_annotation(ret_ann);
                    check_assignable(&kt, Some(ret_ann), &vt, "return value", out);
                }
            }
            for e in exprs {
                validate_expr(e, ctx, out);
            }
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            for (c, b) in branches {
                validate_expr(c, ctx, out);
                let mut bctx = ctx.child();
                validate_stmts(b, &mut bctx, current_ret, out);
            }
            if let Some(b) = else_body {
                let mut bctx = ctx.child();
                validate_stmts(b, &mut bctx, current_ret, out);
            }
        }
        Stmt::IfBind {
            expr,
            when_cond,
            then_body,
            else_body,
            ..
        } => {
            validate_expr(expr, ctx, out);
            if let Some(c) = when_cond {
                validate_expr(c, ctx, out);
            }
            let mut bctx = ctx.child();
            validate_stmts(then_body, &mut bctx, current_ret, out);
            if let Some(b) = else_body {
                let mut bctx = ctx.child();
                validate_stmts(b, &mut bctx, current_ret, out);
            }
        }
        Stmt::For {
            iter,
            when_clause,
            body,
            ..
        } => {
            validate_expr(iter, ctx, out);
            if let Some(w) = when_clause {
                match w {
                    WhenClause::Cond(e) | WhenClause::Bind { expr: e, .. } => {
                        validate_expr(e, ctx, out)
                    }
                }
            }
            let mut bctx = ctx.child();
            validate_stmts(body, &mut bctx, current_ret, out);
        }
        Stmt::While { cond, body } => {
            validate_expr(cond, ctx, out);
            let mut bctx = ctx.child();
            validate_stmts(body, &mut bctx, current_ret, out);
        }
        Stmt::Repeat { body, cond } => {
            let mut bctx = ctx.child();
            validate_stmts(body, &mut bctx, current_ret, out);
            validate_expr(cond, ctx, out);
        }
        Stmt::Do { body } => {
            let mut bctx = ctx.child();
            validate_stmts(body, &mut bctx, current_ret, out);
        }
        Stmt::Call(e) | Stmt::Give(e) => validate_expr(e, ctx, out),
        Stmt::Class(c) => {
            // Validate method bodies with their signatures.
            for m in &c.members {
                match m {
                    ClassMember::Method(md) => {
                        let mut mctx = ctx.child();
                        for p in &md.params {
                            let kt =
                                p.ty.as_deref()
                                    .map(known_from_annotation)
                                    .unwrap_or(KnownType::Unknown);
                            mctx.vars.insert(p.name.clone(), (p.ty.clone(), kt));
                        }
                        validate_stmts(&md.body, &mut mctx, md.return_type.as_deref(), out);
                    }
                    ClassMember::Field(f) => {
                        if let (Some(ann), Some(def)) = (&f.ty, &f.default) {
                            let kt = known_from_annotation(ann);
                            let vt = expr_known_type(def, ctx);
                            check_assignable(
                                &kt,
                                Some(ann),
                                &vt,
                                &format!("field `{}` default", f.name),
                                out,
                            );
                        }
                        if let Some(d) = &f.default {
                            validate_expr(d, ctx, out);
                        }
                    }
                }
            }
        }
        Stmt::Trait(t) => {
            for m in &t.members {
                match m {
                    ClassMember::Method(md) => {
                        let mut mctx = ctx.child();
                        for p in &md.params {
                            let kt =
                                p.ty.as_deref()
                                    .map(known_from_annotation)
                                    .unwrap_or(KnownType::Unknown);
                            mctx.vars.insert(p.name.clone(), (p.ty.clone(), kt));
                        }
                        validate_stmts(&md.body, &mut mctx, md.return_type.as_deref(), out);
                    }
                    ClassMember::Field(f) => {
                        if let (Some(ann), Some(def)) = (&f.ty, &f.default) {
                            let kt = known_from_annotation(ann);
                            let vt = expr_known_type(def, ctx);
                            check_assignable(
                                &kt,
                                Some(ann),
                                &vt,
                                &format!("field `{}` default", f.name),
                                out,
                            );
                        }
                        if let Some(d) = &f.default {
                            validate_expr(d, ctx, out);
                        }
                    }
                }
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn validate_expr(expr: &Expr, ctx: &TypeCtx, out: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Call { func, args, .. } => {
            // Check against known function signatures.
            if let Expr::Ident(name) = func.as_ref() {
                if let Some(sig) = ctx.funcs.get(name).cloned() {
                    for (i, arg) in args.iter().enumerate() {
                        if let Some((Some(ann), kt)) = sig.params.get(i) {
                            let vt = expr_known_type(arg, ctx);
                            check_assignable(
                                kt,
                                Some(ann),
                                &vt,
                                &format!("argument {} of `{}`", i + 1, name),
                                out,
                            );
                        }
                    }
                }
            }
            validate_expr(func, ctx, out);
            for a in args {
                validate_expr(a, ctx, out);
            }
        }
        Expr::Function {
            params,
            body,
            return_type,
        } => {
            let mut fctx = ctx.child();
            for p in params {
                let kt =
                    p.ty.as_deref()
                        .map(known_from_annotation)
                        .unwrap_or(KnownType::Unknown);
                fctx.vars.insert(p.name.clone(), (p.ty.clone(), kt));
            }
            validate_stmts(body, &mut fctx, return_type.as_deref(), out);
        }
        Expr::Binary { left, right, .. } => {
            validate_expr(left, ctx, out);
            validate_expr(right, ctx, out);
        }
        Expr::Unary { expr, .. } | Expr::Paren(expr) | Expr::Try(expr) => {
            validate_expr(expr, ctx, out)
        }
        Expr::Index(a, b) => {
            validate_expr(a, ctx, out);
            validate_expr(b, ctx, out);
        }
        Expr::Field(a, _) => validate_expr(a, ctx, out),
        Expr::Table(fields) => {
            for f in fields {
                match f {
                    TableField::Named(_, e) | TableField::Positional(e) => {
                        validate_expr(e, ctx, out)
                    }
                    TableField::Computed(k, v) => {
                        validate_expr(k, ctx, out);
                        validate_expr(v, ctx, out);
                    }
                }
            }
        }
        Expr::Bundle(items) => {
            for i in items {
                validate_expr(i, ctx, out);
            }
        }
        Expr::OrReturn { value, fallback } => {
            validate_expr(value, ctx, out);
            validate_expr(fallback, ctx, out);
        }
        Expr::DoBlock { body, give } => {
            let mut bctx = ctx.child();
            validate_stmts(body, &mut bctx, None, out);
            validate_expr(give, ctx, out);
        }
        Expr::TableComp {
            iter,
            when_cond,
            give,
            ..
        } => {
            validate_expr(iter, ctx, out);
            if let Some(c) = when_cond {
                validate_expr(c, ctx, out);
            }
            validate_expr(give, ctx, out);
        }
        Expr::Is { value, .. } => validate_expr(value, ctx, out),
        _ => {}
    }
}
