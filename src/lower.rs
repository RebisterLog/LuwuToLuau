use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::luau::*;

#[derive(Default, Clone)]
struct Usage {
    fields: bool,
    member: bool,
    name: bool,
    of: bool,
    isinstance: bool,
    implements: bool,
}

#[derive(Clone)]
struct TryBinding {
    ok: String,
    value: String,
}

pub struct Lowerer {
    traits: HashMap<String, TraitDecl>,
    usage: Usage,
    helpers: HashSet<String>,
    none_used: bool,
    none_declared: bool,
    none_returning: HashSet<String>,
    maybe_none: HashSet<String>,
    try_bindings: HashMap<String, TryBinding>,
    temp_counter: usize,
}

impl Lowerer {
    pub fn new(program: &Program) -> Self {
        let mut traits = HashMap::new();
        collect_traits(&program.stmts, &mut traits);
        let mut usage = Usage::default();
        scan_stmts(&program.stmts, &mut usage);
        let mut none_returning = HashSet::new();
        collect_none_returning(&program.stmts, &mut none_returning);
        let none_declared = program.stmts.iter().any(|s| match s {
            Stmt::Decl { patterns, .. } => patterns.iter().any(|p| matches!(p, Pattern::Name(n) if n == "none")),
            _ => false,
        });
        Self {
            traits,
            usage,
            helpers: HashSet::new(),
            none_used: false,
            none_declared,
            none_returning,
            maybe_none: HashSet::new(),
            try_bindings: HashMap::new(),
            temp_counter: 0,
        }
    }

    pub fn lower(&mut self, program: &Program) -> Result<String, String> {
        let mut out = Vec::new();
        for stmt in &program.stmts {
            out.extend(self.lower_stmt(stmt)?);
        }

        let mut prefix = Vec::new();
        if self.none_used && !self.none_declared {
            prefix.push(LStmt::raw(
                "const none = setmetatable({}, {\n    __tostring = function()\n        return \"none\"\n    end,\n})",
            ));
        }
        if !self.helpers.is_empty() {
            let mut helper_src = String::new();
            for name in [
                "isinstance",
                "of",
                "name",
                "fields",
                "member",
                "implements",
                "table_drop",
            ] {
                if self.helpers.contains(name) {
                    helper_src.push_str(&helper_source(name));
                    helper_src.push('\n');
                }
            }
            prefix.push(LStmt::raw(helper_src.trim_end().to_string()));
        }
        prefix.extend(out);
        Ok(render_program(&prefix))
    }

    fn temp(&mut self, hint: &str) -> String {
        self.temp_counter += 1;
        format!("__luwu_{}_{}", hint, self.temp_counter)
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<Vec<LStmt>, String> {
        match stmt {
            Stmt::Decl {
                kind,
                patterns,
                exprs,
                ..
            } => self.lower_decl(kind, patterns, exprs),
            Stmt::Function {
                kind,
                name,
                params,
                body,
                ..
            } => {
                let (params_out, defaults) = self.lower_params(params)?;
                let body_out = self.with_function_scope(|this| {
                    let mut b = defaults;
                    b.extend(this.lower_block(body)?);
                    Ok(b)
                })?;
                let prefix = match kind {
                    BindingKind::Const => format!("const function {name}"),
                    BindingKind::Local => format!("local function {name}"),
                };
                Ok(vec![LStmt::Function {
                    prefix,
                    params: params_out,
                    body: body_out,
                }])
            }
            Stmt::Assign { targets, op, exprs } => {
                let mut out = Vec::new();
                let mut t = Vec::new();
                for target in targets {
                    t.push(self.lower_expr(target)?);
                }
                let mut e = Vec::new();
                for expr in exprs {
                    e.push(self.lower_expr(expr)?);
                }
                // Keep the maybe-none flow simple for plain single assignments.
                if op.is_none() && t.len() == 1 && e.len() == 1 {
                    if let LExpr::Ident(name) = &t[0] {
                        if self.expr_may_be_none(&exprs[0]) {
                            self.maybe_none.insert(name.clone());
                        }
                    }
                }
                out.push(LStmt::Assign {
                    targets: t,
                    op: op.clone(),
                    exprs: e,
                });
                Ok(out)
            }
            Stmt::Call(expr) => Ok(vec![LStmt::Call(self.lower_expr(expr)?)]),
            Stmt::Return(exprs) => {
                let mut out = Vec::new();
                for expr in exprs {
                    out.push(self.lower_expr(expr)?);
                }
                Ok(vec![LStmt::Return(out)])
            }
            Stmt::If {
                branches,
                else_body,
            } => self.lower_if(branches, else_body.as_ref()),
            Stmt::IfBind {
                name,
                expr,
                when_cond,
                then_body,
                else_body,
            } => self.lower_if_bind(name, expr, when_cond.as_ref(), then_body, else_body.as_ref()),
            Stmt::For {
                patterns,
                iter,
                when_clause,
                body,
            } => self.lower_for(patterns, iter, when_clause.as_ref(), body),
            Stmt::While { cond, body } => Ok(vec![LStmt::While {
                cond: self.lower_expr(cond)?,
                body: self.lower_block(body)?,
            }]),
            Stmt::Repeat { body, cond } => Ok(vec![LStmt::Repeat {
                body: self.lower_block(body)?,
                cond: self.lower_expr(cond)?,
            }]),
            Stmt::Do { body } => Ok(vec![LStmt::Do {
                body: self.lower_block(body)?,
            }]),
            Stmt::Give(expr) => Ok(vec![LStmt::Return(vec![self.lower_expr(expr)?])]),
            Stmt::Break => Ok(vec![LStmt::Break]),
            Stmt::Continue => Ok(vec![LStmt::Continue]),
            Stmt::Class(class) => self.lower_class(class),
            Stmt::Trait(trait_decl) => self.lower_trait(trait_decl),
        }
    }

    fn with_function_scope<F, T>(&mut self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut Self) -> Result<T, String>,
    {
        let saved_none = self.maybe_none.clone();
        let saved_try = self.try_bindings.clone();
        self.maybe_none.clear();
        self.try_bindings.clear();
        let result = f(self);
        self.maybe_none = saved_none;
        self.try_bindings = saved_try;
        result
    }

    fn lower_block(&mut self, body: &[Stmt]) -> Result<Vec<LStmt>, String> {
        let mut out = Vec::new();
        for stmt in body {
            out.extend(self.lower_stmt(stmt)?);
        }
        Ok(out)
    }

    fn lower_decl(
        &mut self,
        kind: &BindingKind,
        patterns: &[Pattern],
        exprs: &[Expr],
    ) -> Result<Vec<LStmt>, String> {
        let mut out = Vec::new();

        // `const x = try f()` is lowered to a pcall pair.
        if patterns.len() == 1 && exprs.len() == 1 {
            if let (Pattern::Name(name), Expr::Try(inner)) = (&patterns[0], &exprs[0]) {
                let (ok, value) = self.lower_try(inner, name);
                out.push(match kind {
                    BindingKind::Const => LStmt::Const {
                        names: vec![ok.clone(), value.clone()],
                        exprs: self.lower_try_call(inner)?,
                    },
                    BindingKind::Local => LStmt::Local {
                        names: vec![ok.clone(), value.clone()],
                        exprs: self.lower_try_call(inner)?,
                    },
                });
                self.try_bindings.insert(name.clone(), TryBinding { ok, value });
                return Ok(out);
            }
        }

        // `const x = y or return z` evaluates y once, then returns on nil.
        if patterns.len() == 1 && exprs.len() == 1 {
            if let Expr::OrReturn { value, fallback } = &exprs[0] {
                let name = match &patterns[0] {
                    Pattern::Name(n) => n.clone(),
                    _ => return Err("`or return` only supports a simple name".into()),
                };
                // The value can be `none` instead of `nil`: then the guard has
                // to compare against the sentinel.
                let empty = if self.expr_may_be_none(value) {
                    self.none_used = true;
                    LExpr::ident("none")
                } else {
                    LExpr::Nil
                };
                if self.expr_may_be_none(value) {
                    self.maybe_none.insert(name.clone());
                }
                out.push(match kind {
                    BindingKind::Const => LStmt::Const {
                        names: vec![name.clone()],
                        exprs: vec![self.lower_expr(value)?],
                    },
                    BindingKind::Local => LStmt::Local {
                        names: vec![name.clone()],
                        exprs: vec![self.lower_expr(value)?],
                    },
                });
                out.push(LStmt::If {
                    cond: LExpr::Binary {
                        op: "==".into(),
                        left: Box::new(LExpr::ident(name.clone())),
                        right: Box::new(empty),
                    },
                    then_body: vec![LStmt::Return(vec![self.lower_expr(fallback)?])],
                    else_body: None,
                });
                return Ok(out);
            }
        }

        // Table comprehensions become a fresh table plus a loop.
        if patterns.len() == 1 && exprs.len() == 1 {
            if let (Pattern::Name(name), Expr::TableComp {
                patterns: vars,
                iter,
                when_cond,
                give,
            }) = (&patterns[0], &exprs[0])
            {
                out.push(match kind {
                    BindingKind::Const => LStmt::Const {
                        names: vec![name.clone()],
                        exprs: vec![LExpr::Table(Vec::new())],
                    },
                    BindingKind::Local => LStmt::Local {
                        names: vec![name.clone()],
                        exprs: vec![LExpr::Table(Vec::new())],
                    },
                });
                let iter_out = self.lower_expr(iter)?;
                let (vars_out, destructure) = self.loop_vars(vars)?;
                let mut body = destructure;
                let insert = LStmt::Call(LExpr::call(
                    LExpr::field(LExpr::ident("table"), "insert"),
                    vec![LExpr::ident(name.clone()), self.lower_expr(give)?],
                ));
                if let Some(cond) = when_cond {
                    let cond = self.lower_cond(cond)?;
                    body.push(LStmt::If {
                        cond,
                        then_body: vec![insert],
                        else_body: None,
                    });
                } else {
                    body.push(insert);
                }
                out.push(LStmt::For {
                    vars: vars_out,
                    iter: iter_out,
                    body,
                });
                return Ok(out);
            }
        }

        let mut names = Vec::new();
        let mut values = Vec::new();
        let mut prelude = Vec::new();

        for (i, pattern) in patterns.iter().enumerate() {
            let value = exprs.get(i);
            match pattern {
                Pattern::Name(name) => {
                    names.push(name.clone());
                    if let Some(value) = value {
                        values.push(self.lower_expr(value)?);
                        if self.expr_may_be_none(value) {
                            self.maybe_none.insert(name.clone());
                        }
                    }
                }
                Pattern::Table(fields) => {
                    let (tmp, pre) = self.eval_once(value.unwrap_or(&Expr::Nil), "destructure")?;
                    prelude.extend(pre);
                    for f in fields {
                        names.push(f.clone());
                        values.push(LExpr::field(tmp.clone(), f.clone()));
                    }
                }
                Pattern::Bundle(fields) => {
                    let (tmp, pre) = self.eval_once(value.unwrap_or(&Expr::Nil), "bundle")?;
                    prelude.extend(pre);
                    for (idx, f) in fields.iter().enumerate() {
                        names.push(f.clone());
                        values.push(LExpr::index(
                            tmp.clone(),
                            LExpr::Number((idx + 1).to_string()),
                        ));
                    }
                }
            }
        }

        out.extend(prelude);
        out.push(match kind {
            BindingKind::Const => LStmt::Const { names, exprs: values },
            BindingKind::Local => LStmt::Local { names, exprs: values },
        });
        Ok(out)
    }

    fn eval_once(
        &mut self,
        expr: &Expr,
        hint: &str,
    ) -> Result<(LExpr, Vec<LStmt>), String> {
        match expr {
            Expr::Ident(_) | Expr::Field(_, _) | Expr::Index(_, _) => {
                Ok((self.lower_expr(expr)?, Vec::new()))
            }
            _ => {
                let name = self.temp(hint);
                let value = self.lower_expr(expr)?;
                Ok((
                    LExpr::ident(name.clone()),
                    vec![LStmt::Const {
                        names: vec![name],
                        exprs: vec![value],
                    }],
                ))
            }
        }
    }

    fn lower_if(
        &mut self,
        branches: &[(Expr, Vec<Stmt>)],
        else_body: Option<&Vec<Stmt>>,
    ) -> Result<Vec<LStmt>, String> {
        // If the first condition is `x is Ok.(v)` / `Err.(e)`, lower it to a
        // pcall flag check and bind the payload at the start of the branch.
        if let Some((first_cond, first_body)) = branches.first() {
            if let Expr::Is {
                value,
                kind,
                binding,
            } = first_cond
            {
                let mut then_body = Vec::new();
                let cond = self.lower_is_cond(value, kind, binding, &mut then_body)?;
                then_body.extend(self.lower_block(first_body)?);
                let else_lowered = if let Some(e) = else_body {
                    Some(self.lower_block(e)?)
                } else {
                    None
                };
                return Ok(vec![LStmt::If {
                    cond,
                    then_body,
                    else_body: else_lowered,
                }]);
            }
        }

        // Build nested ifs for elseif chains.
        let mut current_else = else_body.map(|b| self.lower_block(b)).transpose()?;
        for (cond, body) in branches.iter().rev() {
            let cond = self.lower_cond(cond)?;
            let then_body = self.lower_block(body)?;
            current_else = Some(vec![LStmt::If {
                cond,
                then_body,
                else_body: current_else,
            }]);
        }
        Ok(current_else.unwrap_or_default())
    }

    fn lower_if_bind(
        &mut self,
        name: &str,
        expr: &Expr,
        when_cond: Option<&Expr>,
        then_body: &[Stmt],
        else_body: Option<&Vec<Stmt>>,
    ) -> Result<Vec<LStmt>, String> {
        let mut out = Vec::new();
        let value = self.lower_expr(expr)?;
        out.push(LStmt::Const {
            names: vec![name.to_string()],
            exprs: vec![value],
        });
        if self.expr_may_be_none(expr) {
            self.maybe_none.insert(name.to_string());
        }
        let mut cond = LExpr::Binary {
            op: "~=".into(),
            left: Box::new(LExpr::ident(name)),
            right: Box::new(if self.expr_may_be_none(expr) {
                LExpr::ident("none")
            } else {
                LExpr::Nil
            }),
        };
        if let Some(w) = when_cond {
            cond = LExpr::Binary {
                op: "and".into(),
                left: Box::new(cond),
                right: Box::new(self.lower_expr(w)?),
            };
        }
        out.push(LStmt::If {
            cond,
            then_body: self.lower_block(then_body)?,
            else_body: else_body.map(|b| self.lower_block(b)).transpose()?,
        });
        Ok(out)
    }

    fn lower_for(
        &mut self,
        patterns: &[Pattern],
        iter: &Expr,
        when_clause: Option<&WhenClause>,
        body: &[Stmt],
    ) -> Result<Vec<LStmt>, String> {
        let iter_out = self.lower_expr(iter)?;
        let (mut vars, mut destructure) = self.loop_vars(patterns)?;
        if vars.len() == 1 && !is_iterator_call(iter) {
            vars.insert(0, "_".to_string());
        }
        let mut body_out = Vec::new();
        if let Some(clause) = when_clause {
            match clause {
                WhenClause::Cond(c) => {
                    let cond = self.lower_cond(c)?;
                    body_out.push(LStmt::If {
                        cond,
                        then_body: self.lower_block(body)?,
                        else_body: None,
                    });
                    body_out = {
                        let mut v = destructure;
                        v.extend(body_out);
                        v
                    };
                    return Ok(vec![LStmt::For {
                        vars,
                        iter: iter_out,
                        body: body_out,
                    }]);
                }
                WhenClause::Bind { name, expr } => {
                    destructure.push(LStmt::Const {
                        names: vec![name.clone()],
                        exprs: vec![self.lower_expr(expr)?],
                    });
                    let cond = LExpr::Binary {
                        op: "~=".into(),
                        left: Box::new(LExpr::ident(name)),
                        right: Box::new(LExpr::Nil),
                    };
                    body_out.push(LStmt::If {
                        cond,
                        then_body: self.lower_block(body)?,
                        else_body: None,
                    });
                }
            }
        } else {
            body_out = self.lower_block(body)?;
        }
        destructure.extend(body_out);
        Ok(vec![LStmt::For {
            vars,
            iter: iter_out,
            body: destructure,
        }])
    }

    fn loop_vars(&mut self, patterns: &[Pattern]) -> Result<(Vec<String>, Vec<LStmt>), String> {
        let mut vars = Vec::new();
        let mut prelude = Vec::new();
        for pattern in patterns {
            match pattern {
                Pattern::Name(n) => vars.push(n.clone()),
                Pattern::Table(fields) => {
                    let tmp = self.temp("loop");
                    vars.push(tmp.clone());
                    for f in fields {
                        prelude.push(LStmt::Const {
                            names: vec![f.clone()],
                            exprs: vec![LExpr::field(LExpr::ident(tmp.clone()), f.clone())],
                        });
                    }
                }
                Pattern::Bundle(fields) => {
                    let tmp = self.temp("loop");
                    vars.push(tmp.clone());
                    for (i, f) in fields.iter().enumerate() {
                        prelude.push(LStmt::Const {
                            names: vec![f.clone()],
                            exprs: vec![LExpr::index(
                                LExpr::ident(tmp.clone()),
                                LExpr::Number((i + 1).to_string()),
                            )],
                        });
                    }
                }
            }
        }
        Ok((vars, prelude))
    }

    fn lower_cond(&mut self, expr: &Expr) -> Result<LExpr, String> {
        // `if found then` for a value that can be `none` means "not none".
        if let Expr::Ident(name) = expr {
            if self.maybe_none.contains(name) {
                return Ok(LExpr::Binary {
                    op: "~=".into(),
                    left: Box::new(LExpr::ident(name)),
                    right: Box::new(LExpr::ident("none")),
                });
            }
        }
        self.lower_expr(expr)
    }

    fn lower_is_cond(
        &mut self,
        value: &Expr,
        kind: &str,
        binding: &str,
        prelude: &mut Vec<LStmt>,
    ) -> Result<LExpr, String> {
        let base = match value {
            Expr::Ident(name) => self
                .try_bindings
                .get(name)
                .cloned()
                .unwrap_or(TryBinding {
                    ok: name.clone(),
                    value: name.clone(),
                }),
            _ => TryBinding {
                ok: "true".into(),
                value: "nil".into(),
            },
        };
        let (cond_op, payload) = if kind == "Ok" {
            ("", base.value)
        } else {
            ("not ", base.value)
        };
        prelude.push(LStmt::Const {
            names: vec![binding.to_string()],
            exprs: vec![LExpr::ident(payload)],
        });
        if cond_op.is_empty() {
            Ok(LExpr::ident(base.ok))
        } else {
            Ok(LExpr::Unary {
                op: "not".into(),
                expr: Box::new(LExpr::ident(base.ok)),
            })
        }
    }

    fn lower_try(&mut self, _expr: &Expr, binding: &str) -> (String, String) {
        (format!("{binding}_ok"), format!("{binding}_value"))
    }

    fn lower_try_call(&mut self, expr: &Expr) -> Result<Vec<LExpr>, String> {
        if let Expr::Call { func, args, .. } = expr {
            let f = self.lower_expr(func)?;
            let mut call_args = vec![f];
            for a in args {
                call_args.push(self.lower_expr(a)?);
            }
            Ok(vec![LExpr::call(LExpr::ident("pcall"), call_args)])
        } else {
            let e = self.lower_expr(expr)?;
            let body = vec![LStmt::Return(vec![e])];
            Ok(vec![LExpr::Iife {
                params: Vec::new(),
                body,
                args: Vec::new(),
            }])
        }
    }

    fn expr_may_be_none(&self, expr: &Expr) -> bool {
        match expr {
            Expr::None => true,
            Expr::Paren(e) => self.expr_may_be_none(e),
            Expr::Call { func, .. } => match func.as_ref() {
                Expr::Ident(n) => self.none_returning.contains(n),
                _ => false,
            },
            _ => false,
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<LExpr, String> {
        match expr {
            Expr::Nil => Ok(LExpr::Nil),
            Expr::Bool(b) => Ok(LExpr::Bool(*b)),
            Expr::Number(n) => Ok(LExpr::Number(n.clone())),
            Expr::Str(s) => Ok(LExpr::Str(s.clone())),
            Expr::Backtick(s) => Ok(LExpr::Backtick(s.clone())),
            Expr::Ident(n) => Ok(LExpr::ident(n.clone())),
            Expr::Paren(e) => Ok(LExpr::Paren(Box::new(self.lower_expr(e)?))),
            Expr::None => {
                self.none_used = true;
                Ok(LExpr::ident("none"))
            }
            Expr::Vararg => Ok(LExpr::Vararg),
            Expr::Table(fields) => {
                let mut out = Vec::new();
                for f in fields {
                    match f {
                        TableField::Named(k, v) => out.push(LField::Named(k.clone(), self.lower_expr(v)?)),
                        TableField::Positional(v) => out.push(LField::Positional(self.lower_expr(v)?)),
                        TableField::Computed(k, v) => out.push(LField::Computed(
                            self.lower_expr(k)?,
                            self.lower_expr(v)?,
                        )),
                    }
                }
                Ok(LExpr::Table(out))
            }
            Expr::Bundle(items) => {
                let mut out = Vec::new();
                for i in items {
                    out.push(LField::Positional(self.lower_expr(i)?));
                }
                Ok(LExpr::Table(out))
            }
            Expr::Restructure(names) => {
                let mut out = Vec::new();
                for n in names {
                    out.push(LField::Named(n.clone(), LExpr::ident(n.clone())));
                }
                Ok(LExpr::Table(out))
            }
            Expr::Index(base, idx) => Ok(LExpr::index(
                self.lower_expr(base)?,
                self.lower_expr(idx)?,
            )),
            Expr::Field(base, name) => Ok(LExpr::field(self.lower_expr(base)?, name.clone())),
            Expr::Call {
                func,
                args,
                method,
            } => self.lower_call(func, args, *method),
            Expr::Unary { op, expr } => Ok(LExpr::Unary {
                op: op.clone(),
                expr: Box::new(self.lower_expr(expr)?),
            }),
            Expr::Binary { op, left, right } => Ok(LExpr::Binary {
                op: op.clone(),
                left: Box::new(self.lower_expr(left)?),
                right: Box::new(self.lower_expr(right)?),
            }),
            Expr::OrReturn { .. } => Err("`or return` is only supported in a declaration".into()),
            Expr::Try(_) => Err("`try` is only supported as a declaration initializer".into()),
            Expr::DoBlock { body, give } => {
                let mut b = self.with_function_scope(|this| this.lower_block(body))?;
                b.push(LStmt::Return(vec![self.lower_expr(give)?]));
                Ok(LExpr::Iife {
                    params: Vec::new(),
                    body: b,
                    args: Vec::new(),
                })
            }
            Expr::TableComp {
                patterns,
                iter,
                when_cond,
                give,
            } => {
                let tmp = self.temp("comp");
                let iter_out = self.lower_expr(iter)?;
                let (vars, destructure) = self.loop_vars(patterns)?;
                let mut body = destructure;
                let insert = LStmt::Call(LExpr::call(
                    LExpr::field(LExpr::ident("table"), "insert"),
                    vec![LExpr::ident(tmp.clone()), self.lower_expr(give)?],
                ));
                if let Some(cond) = when_cond {
                    body.push(LStmt::If {
                        cond: self.lower_cond(cond)?,
                        then_body: vec![insert],
                        else_body: None,
                    });
                } else {
                    body.push(insert);
                }
                let stmt = vec![
                    LStmt::Local {
                        names: vec![tmp.clone()],
                        exprs: vec![LExpr::Table(Vec::new())],
                    },
                    LStmt::For {
                        vars,
                        iter: iter_out,
                        body,
                    },
                    LStmt::Return(vec![LExpr::ident(tmp)]),
                ];
                Ok(LExpr::Iife {
                    params: Vec::new(),
                    body: stmt,
                    args: Vec::new(),
                })
            }
            Expr::Is { .. } => Err("`is Ok/Err` is only supported in an if condition".into()),
            Expr::Function { params, body, .. } => {
                let (p, defaults) = self.lower_params(params)?;
                let mut b = defaults;
                b.extend(self.with_function_scope(|this| this.lower_block(body))?);
                Ok(LExpr::Function { params: p, body: b })
            }
        }
    }

    fn lower_call(
        &mut self,
        func: &Expr,
        args: &[Expr],
        method: bool,
    ) -> Result<LExpr, String> {
        // Luwu's standard-library extensions.
        if let Expr::Field(base, name) = func {
            if let Expr::Ident(ns) = base.as_ref() {
                if ns == "table" && name == "drop" {
                    self.helpers.insert("table_drop".into());
                    let mut call_args = Vec::new();
                    for a in args {
                        call_args.push(self.lower_expr(a)?);
                    }
                    return Ok(LExpr::call(
                        LExpr::ident("__luwu_table_drop"),
                        call_args,
                    ));
                }
                if ns == "class" {
                    return self.lower_class_call(name, args);
                }
            }
        }

        let f = self.lower_expr(func)?;
        let mut call_args = Vec::new();
        for a in args {
            if let Expr::Ident(n) = a {
                if let Some(tb) = self.try_bindings.get(n) {
                    call_args.push(LExpr::ident(tb.ok.clone()));
                    call_args.push(LExpr::ident(tb.value.clone()));
                    continue;
                }
            }
            call_args.push(self.lower_expr(a)?);
        }
        Ok(LExpr::Call {
            func: Box::new(f),
            args: call_args,
            method,
        })
    }

    fn lower_class_call(&mut self, name: &str, args: &[Expr]) -> Result<LExpr, String> {
        match name {
            "isinstance" => {
                self.helpers.insert("isinstance".into());
                let mut out = Vec::new();
                for a in args {
                    out.push(self.lower_expr(a)?);
                }
                Ok(LExpr::call(LExpr::ident("__luwu_isinstance"), out))
            }
            "of" => {
                self.helpers.insert("of".into());
                let mut out = Vec::new();
                for a in args {
                    out.push(self.lower_expr(a)?);
                }
                Ok(LExpr::call(LExpr::ident("__luwu_class_of"), out))
            }
            "name" => {
                self.helpers.insert("name".into());
                let mut out = Vec::new();
                for a in args {
                    out.push(self.lower_expr(a)?);
                }
                Ok(LExpr::call(LExpr::ident("__luwu_class_name"), out))
            }
            "fields" => {
                self.helpers.insert("fields".into());
                let mut out = Vec::new();
                for a in args {
                    out.push(self.lower_expr(a)?);
                }
                Ok(LExpr::call(LExpr::ident("__luwu_fields"), out))
            }
            "member" => {
                self.helpers.insert("member".into());
                let mut out = Vec::new();
                for a in args {
                    out.push(self.lower_expr(a)?);
                }
                Ok(LExpr::call(LExpr::ident("__luwu_member"), out))
            }
            "implements" => {
                self.helpers.insert("implements".into());
                let mut out = Vec::new();
                let mut trait_name = None;
                for (i, a) in args.iter().enumerate() {
                    if i == 1 {
                        if let Expr::Ident(n) = a {
                            if self.traits.contains_key(n) {
                                trait_name = Some(n.clone());
                                out.push(LExpr::Str(format!("{:?}", n)));
                                continue;
                            }
                        }
                    }
                    out.push(self.lower_expr(a)?);
                }
                let _ = trait_name;
                Ok(LExpr::call(LExpr::ident("__luwu_implements"), out))
            }
            _ => {
                let f = LExpr::field(LExpr::ident("class"), name);
                let mut call_args = Vec::new();
                for a in args {
                    call_args.push(self.lower_expr(a)?);
                }
                Ok(LExpr::Call {
                    func: Box::new(f),
                    args: call_args,
                    method: false,
                })
            }
        }
    }

    fn lower_params(&mut self, params: &[Param]) -> Result<(Vec<String>, Vec<LStmt>), String> {
        let mut names = Vec::new();
        let mut defaults = Vec::new();
        for p in params {
            names.push(p.name.clone());
            if let Some(default) = &p.default {
                defaults.push(LStmt::If {
                    cond: LExpr::Binary {
                        op: "==".into(),
                        left: Box::new(LExpr::ident(p.name.clone())),
                        right: Box::new(LExpr::Nil),
                    },
                    then_body: vec![LStmt::Assign {
                        targets: vec![LExpr::ident(p.name.clone())],
                        op: None,
                        exprs: vec![self.lower_expr(default)?],
                    }],
                    else_body: None,
                });
            }
        }
        Ok((names, defaults))
    }

    fn lower_method(
        &mut self,
        owner: &str,
        method: &MethodDecl,
    ) -> Result<Vec<LStmt>, String> {
        if method.is_expect {
            return Ok(Vec::new());
        }
        let has_self = !method.is_static
            && method.params.first().is_some_and(|p| p.name == "self");
        let visible_params: Vec<Param> = if has_self {
            method.params[1..].to_vec()
        } else {
            method.params.clone()
        };
        let (params, defaults) = self.lower_params(&visible_params)?;
        let body = self.with_function_scope(|this| {
            let mut b = defaults;
            b.extend(this.lower_block(&method.body)?);
            Ok(b)
        })?;
        let prefix = if has_self {
            format!("function {owner}:{}", method.name)
        } else {
            format!("function {owner}.{}", method.name)
        };
        Ok(vec![LStmt::Function {
            prefix,
            params,
            body,
        }])
    }

    fn lower_class(&mut self, class: &ClassDecl) -> Result<Vec<LStmt>, String> {
        let mut out = Vec::new();

        let fields: Vec<&FieldDecl> = class
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Field(f) if !f.is_expect => Some(f),
                _ => None,
            })
            .collect();
        let methods: Vec<&MethodDecl> = class
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(m) if !m.is_expect => Some(m),
                _ => None,
            })
            .collect();
        let has_init = methods.iter().any(|m| m.name == "__init");

        // Instantiate parametrized traits first and remember their binding.
        let mut trait_instances: HashMap<String, String> = HashMap::new();
        for imp in &class.implements {
            if let Some(trait_decl) = self.traits.get(&imp.name) {
                if !trait_decl.params.is_empty() {
                    let binding = format!("Stacked{}{}", class.name, imp.name);
                    let mut call_args = Vec::new();
                    for a in &imp.args {
                        call_args.push(self.lower_expr(a)?);
                    }
                    out.push(LStmt::Const {
                        names: vec![binding.clone()],
                        exprs: vec![LExpr::call(LExpr::ident(imp.name.clone()), call_args)],
                    });
                    trait_instances.insert(imp.name.clone(), binding);
                }
            }
        }

        // Fields and methods contributed by implemented traits (including
        // traits pulled in through `needs`). They are needed both for metadata
        // (`class.fields` / `class.member`) and for the constructor/copy pass.
        let mut all_trait_fields: Vec<(String, LExpr)> = Vec::new();
        let mut trait_field_meta: Vec<(String, bool)> = Vec::new();
        for imp in &class.implements {
            for trait_name in trait_closure_names(&imp.name, &self.traits) {
                let Some(td) = self.traits.get(&trait_name).cloned() else {
                    continue;
                };
                let info = TraitInfo::from(&td);
                if let Some(binding) = trait_instances.get(&trait_name) {
                    for p in &td.params {
                        all_trait_fields.push((
                            p.name.clone(),
                            LExpr::field(LExpr::ident(binding.clone()), p.name.clone()),
                        ));
                        trait_field_meta.push((p.name.clone(), p.is_const));
                    }
                    for f in info.instance_fields() {
                        all_trait_fields.push((
                            f.name.clone(),
                            LExpr::field(LExpr::ident(binding.clone()), f.name.clone()),
                        ));
                        trait_field_meta.push((f.name.clone(), f.is_const));
                    }
                } else {
                    for f in info.instance_fields() {
                        if let Some(default) = &f.default {
                            all_trait_fields.push((f.name.clone(), self.lower_expr(default)?));
                        }
                        trait_field_meta.push((f.name.clone(), f.is_const));
                    }
                }
            }
        }

        let mut copied_trait_methods: Vec<(String, LExpr)> = Vec::new();
        {
            let mut taken: HashSet<String> = methods.iter().map(|m| m.name.clone()).collect();
            for imp in &class.implements {
                for trait_name in trait_closure_names(&imp.name, &self.traits) {
                    let Some(td) = self.traits.get(&trait_name).cloned() else {
                        continue;
                    };
                    let source = match trait_instances.get(&trait_name) {
                        Some(binding) => LExpr::ident(binding.clone()),
                        None if td.params.is_empty() => LExpr::ident(trait_name.clone()),
                        None => continue,
                    };
                    for m in TraitInfo::from(&td).copyable_methods() {
                        if taken.insert(m.name.clone()) {
                            copied_trait_methods.push((m.name.clone(), source.clone()));
                        }
                    }
                }
            }
        }

        out.push(LStmt::Const {
            names: vec![class.name.clone()],
            exprs: vec![LExpr::Table(Vec::new())],
        });
        out.push(LStmt::Assign {
            targets: vec![LExpr::field(LExpr::ident(class.name.clone()), "__index")],
            op: None,
            exprs: vec![LExpr::ident(class.name.clone())],
        });

        if self.usage.name {
            out.push(LStmt::Assign {
                targets: vec![LExpr::field(LExpr::ident(class.name.clone()), "__name")],
                op: None,
                exprs: vec![LExpr::Str(format!("{:?}", class.name))],
            });
        }
        if self.usage.fields {
            out.push(self.fields_metadata(class, &fields, &trait_field_meta));
        }
        if self.usage.member {
            out.push(self.members_metadata(
                class,
                &fields,
                &methods,
                &trait_field_meta,
                &copied_trait_methods,
            ));
        }
        if self.usage.implements {
            out.push(self.traits_metadata(class));
        }

        // Constructor.
        let own_field_names: HashSet<String> =
            fields.iter().map(|f| f.name.clone()).collect();
        let param_names: HashSet<String> =
            class.params.iter().map(|p| p.name.clone()).collect();
        let trait_field_names: HashSet<String> =
            all_trait_fields.iter().map(|(n, _)| n.clone()).collect();

        let has_default_fields = fields.iter().any(|f| f.default.is_some());
        let is_primary = !class.params.is_empty();
        let is_init_ctor = !is_primary && has_init;
        let is_pod = !is_primary
            && !is_init_ctor
            && !class.has_ctor_parens
            && (has_default_fields || !all_trait_fields.is_empty());

        if is_primary {
            let mut defaults = Vec::new();
            for p in &class.params {
                if let Some(d) = &p.default {
                    defaults.push(LStmt::If {
                        cond: LExpr::Binary {
                            op: "==".into(),
                            left: Box::new(LExpr::ident(p.name.clone())),
                            right: Box::new(LExpr::Nil),
                        },
                        then_body: vec![LStmt::Assign {
                            targets: vec![LExpr::ident(p.name.clone())],
                            op: None,
                            exprs: vec![self.lower_expr(d)?],
                        }],
                        else_body: None,
                    });
                }
            }
            let body = self.primary_ctor_body(class, &fields, &all_trait_fields)?;
            let mut full = defaults;
            full.extend(body);
            out.push(LStmt::Function {
                prefix: format!("function {}.new", class.name),
                params: class
                    .params
                    .iter()
                    .filter(|p| p.name != "...")
                    .map(|p| p.name.clone())
                    .collect(),
                body: full,
            });
        } else if is_init_ctor {
            let mut defaults_table = Vec::new();
            for f in &fields {
                if let Some(d) = &f.default {
                    defaults_table.push(LField::Named(f.name.clone(), self.lower_expr(d)?));
                }
            }
            for (n, v) in &all_trait_fields {
                defaults_table.push(LField::Named(n.clone(), v.clone()));
            }
            let mut body = Vec::new();
            if !defaults_table.is_empty() {
                body.push(LStmt::Const {
                    names: vec!["self".into()],
                    exprs: vec![LExpr::call(
                        LExpr::ident("setmetatable"),
                        vec![LExpr::Table(defaults_table), LExpr::ident(class.name.clone())],
                    )],
                });
            } else {
                body.push(LStmt::Const {
                    names: vec!["self".into()],
                    exprs: vec![LExpr::call(
                        LExpr::ident("setmetatable"),
                        vec![LExpr::Table(Vec::new()), LExpr::ident(class.name.clone())],
                    )],
                });
            }
            body.push(LStmt::Call(LExpr::call(
                LExpr::field(LExpr::ident(class.name.clone()), "__init"),
                vec![LExpr::ident("self"), LExpr::Vararg],
            )));
            body.push(LStmt::Return(vec![LExpr::ident("self")]));
            out.push(LStmt::Function {
                prefix: format!("function {}.new", class.name),
                params: vec!["...".into()],
                body,
            });
        } else if is_pod {
            let mut defaults_table = Vec::new();
            for f in &fields {
                if let Some(d) = &f.default {
                    defaults_table.push(LField::Named(f.name.clone(), self.lower_expr(d)?));
                }
            }
            for (n, v) in &all_trait_fields {
                defaults_table.push(LField::Named(n.clone(), v.clone()));
            }
            let mut body = Vec::new();
            body.push(LStmt::Const {
                names: vec!["self".into()],
                exprs: vec![LExpr::call(
                    LExpr::ident("setmetatable"),
                    vec![LExpr::Table(defaults_table), LExpr::ident(class.name.clone())],
                )],
            });
            if !fields.is_empty() {
                body.push(LStmt::For {
                    vars: vec!["key".into(), "value".into()],
                    iter: LExpr::Binary {
                        op: "or".into(),
                        left: Box::new(LExpr::ident("init")),
                        right: Box::new(LExpr::Table(Vec::new())),
                    },
                    body: vec![LStmt::Assign {
                        targets: vec![LExpr::index(
                            LExpr::ident("self"),
                            LExpr::ident("key"),
                        )],
                        op: None,
                        exprs: vec![LExpr::ident("value")],
                    }],
                });
            }
            body.push(LStmt::Return(vec![LExpr::ident("self")]));
            out.push(LStmt::Function {
                prefix: format!("function {}.new", class.name),
                params: if fields.is_empty() {
                    Vec::new()
                } else {
                    vec!["init".into()]
                },
                body,
            });
        } else {
            out.push(LStmt::Function {
                prefix: format!("function {}.new", class.name),
                params: Vec::new(),
                body: vec![LStmt::Return(vec![LExpr::call(
                    LExpr::ident("setmetatable"),
                    vec![LExpr::Table(Vec::new()), LExpr::ident(class.name.clone())],
                )])],
            });
        }

        // Methods.
        for m in &methods {
            out.extend(self.lower_method(&class.name, m)?);
        }

        // Copy trait methods that the class did not override.
        for (name, source) in &copied_trait_methods {
            if name == "__eq" {
                // Equality must not leak between classes: the trait's `__eq`
                // only answers for objects of the very same class.
                out.push(LStmt::Function {
                    prefix: format!("function {}.__eq", class.name),
                    params: vec!["self".into(), "other".into()],
                    body: vec![
                        LStmt::If {
                            cond: LExpr::Binary {
                                op: "~=".into(),
                                left: Box::new(LExpr::call(
                                    LExpr::ident("getmetatable"),
                                    vec![LExpr::ident("self")],
                                )),
                                right: Box::new(LExpr::call(
                                    LExpr::ident("getmetatable"),
                                    vec![LExpr::ident("other")],
                                )),
                            },
                            then_body: vec![LStmt::Return(vec![LExpr::Bool(false)])],
                            else_body: None,
                        },
                        LStmt::Return(vec![LExpr::call(
                            LExpr::field(source.clone(), "__eq"),
                            vec![LExpr::ident("self"), LExpr::ident("other")],
                        )]),
                    ],
                });
                continue;
            }
            out.push(LStmt::Assign {
                targets: vec![LExpr::field(
                    LExpr::ident(class.name.clone()),
                    name.clone(),
                )],
                op: None,
                exprs: vec![LExpr::field(source.clone(), name.clone())],
            });
        }

        // Callable class table.
        out.push(LStmt::Call(LExpr::call(
            LExpr::ident("setmetatable"),
            vec![
                LExpr::ident(class.name.clone()),
                LExpr::Table(vec![LField::Named(
                    "__call".into(),
                    LExpr::Function {
                        params: vec!["_".into(), "...".into()],
                        body: vec![LStmt::Return(vec![LExpr::call(
                            LExpr::field(LExpr::ident(class.name.clone()), "new"),
                            vec![LExpr::Vararg],
                        )])],
                    },
                )]),
            ],
        )));

        // Run __implemented hooks.
        for imp in &class.implements {
            if let Some(td) = self.traits.get(&imp.name) {
                let info = TraitInfo::from(td);
                if info.has_implemented() {
                    out.push(LStmt::Call(LExpr::call(
                        LExpr::ident("implemented"),
                        vec![LExpr::ident(class.name.clone())],
                    )));
                }
            }
        }

        let _ = (own_field_names, param_names, trait_field_names);
        Ok(out)
    }

    fn primary_ctor_body(
        &mut self,
        class: &ClassDecl,
        fields: &[&FieldDecl],
        trait_fields: &[(String, LExpr)],
    ) -> Result<Vec<LStmt>, String> {
        let mut body = Vec::new();
        body.push(LStmt::Const {
            names: vec!["self".into()],
            exprs: vec![LExpr::call(
                LExpr::ident("setmetatable"),
                vec![LExpr::Table(Vec::new()), LExpr::ident(class.name.clone())],
            )],
        });
        let mut assigned: HashSet<String> = HashSet::new();
        for p in &class.params {
            if p.name == "..." {
                continue;
            }
            body.push(LStmt::Assign {
                targets: vec![LExpr::field(LExpr::ident("self"), p.name.clone())],
                op: None,
                exprs: vec![LExpr::ident(p.name.clone())],
            });
            assigned.insert(p.name.clone());
        }
        for f in fields {
            if assigned.contains(&f.name) {
                continue;
            }
            if let Some(d) = &f.default {
                body.push(LStmt::Assign {
                    targets: vec![LExpr::field(LExpr::ident("self"), f.name.clone())],
                    op: None,
                    exprs: vec![self.lower_expr(d)?],
                });
            }
        }
        for (name, value) in trait_fields {
            if assigned.contains(name) {
                continue;
            }
            body.push(LStmt::Assign {
                targets: vec![LExpr::field(LExpr::ident("self"), name.clone())],
                op: None,
                exprs: vec![value.clone()],
            });
            assigned.insert(name.clone());
        }
        body.push(LStmt::Return(vec![LExpr::ident("self")]));
        Ok(body)
    }

    fn fields_metadata(
        &mut self,
        class: &ClassDecl,
        fields: &[&FieldDecl],
        trait_fields: &[(String, bool)],
    ) -> LStmt {
        let mut entries = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (i, p) in class.params.iter().enumerate() {
            if p.visibility.as_deref() == Some("private") {
                continue;
            }
            seen.insert(p.name.clone());
            entries.push(LField::Positional(LExpr::Table(vec![
                LField::Named("name".into(), LExpr::Str(format!("{:?}", p.name))),
                LField::Named("isconst".into(), LExpr::Bool(p.is_const)),
                LField::Named("position".into(), LExpr::Number((i + 1).to_string())),
            ])));
        }
        for f in fields {
            if f.visibility.as_deref() == Some("private") {
                continue;
            }
            seen.insert(f.name.clone());
            entries.push(LField::Positional(LExpr::Table(vec![
                LField::Named("name".into(), LExpr::Str(format!("{:?}", f.name))),
                LField::Named("isconst".into(), LExpr::Bool(f.is_const)),
                LField::Named("position".into(), LExpr::Nil),
            ])));
        }
        for (name, is_const) in trait_fields {
            if !seen.insert(name.clone()) {
                continue;
            }
            entries.push(LField::Positional(LExpr::Table(vec![
                LField::Named("name".into(), LExpr::Str(format!("{:?}", name))),
                LField::Named("isconst".into(), LExpr::Bool(*is_const)),
                LField::Named("position".into(), LExpr::Nil),
            ])));
        }
        LStmt::Assign {
            targets: vec![LExpr::field(LExpr::ident(class.name.clone()), "fields")],
            op: None,
            exprs: vec![LExpr::Table(entries)],
        }
    }

    fn members_metadata(
        &mut self,
        class: &ClassDecl,
        fields: &[&FieldDecl],
        methods: &[&MethodDecl],
        trait_fields: &[(String, bool)],
        trait_methods: &[(String, LExpr)],
    ) -> LStmt {
        let mut entries = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (i, p) in class.params.iter().enumerate() {
            if p.visibility.as_deref() == Some("private") {
                continue;
            }
            seen.insert(p.name.clone());
            entries.push(LField::Named(
                p.name.clone(),
                LExpr::Table(vec![
                    LField::Named("name".into(), LExpr::Str(format!("{:?}", p.name))),
                    LField::Named("kind".into(), LExpr::Str("\"field\"".into())),
                    LField::Named("isconst".into(), LExpr::Bool(p.is_const)),
                    LField::Named(
                        "visibility".into(),
                        LExpr::Str(format!("{:?}", p.visibility.as_deref().unwrap_or("public"))),
                    ),
                    LField::Named("position".into(), LExpr::Number((i + 1).to_string())),
                ]),
            ));
        }
        for f in fields {
            if f.visibility.as_deref() == Some("private") {
                continue;
            }
            seen.insert(f.name.clone());
            entries.push(LField::Named(
                f.name.clone(),
                LExpr::Table(vec![
                    LField::Named("name".into(), LExpr::Str(format!("{:?}", f.name))),
                    LField::Named("kind".into(), LExpr::Str("\"field\"".into())),
                    LField::Named("isconst".into(), LExpr::Bool(f.is_const)),
                    LField::Named(
                        "visibility".into(),
                        LExpr::Str(format!("{:?}", f.visibility.as_deref().unwrap_or("public"))),
                    ),
                    LField::Named("position".into(), LExpr::Nil),
                ]),
            ));
        }
        for (name, is_const) in trait_fields {
            if !seen.insert(name.clone()) {
                continue;
            }
            entries.push(LField::Named(
                name.clone(),
                LExpr::Table(vec![
                    LField::Named("name".into(), LExpr::Str(format!("{:?}", name))),
                    LField::Named("kind".into(), LExpr::Str("\"field\"".into())),
                    LField::Named("isconst".into(), LExpr::Bool(*is_const)),
                    LField::Named("visibility".into(), LExpr::Str("\"public\"".into())),
                    LField::Named("position".into(), LExpr::Nil),
                ]),
            ));
        }
        for m in methods {
            if m.name.starts_with("__") {
                continue;
            }
            seen.insert(m.name.clone());
            entries.push(LField::Named(
                m.name.clone(),
                LExpr::Table(vec![
                    LField::Named("name".into(), LExpr::Str(format!("{:?}", m.name))),
                    LField::Named("kind".into(), LExpr::Str("\"method\"".into())),
                    LField::Named("isconst".into(), LExpr::Bool(false)),
                    LField::Named(
                        "visibility".into(),
                        LExpr::Str(format!("{:?}", m.visibility.as_deref().unwrap_or("public"))),
                    ),
                    LField::Named("position".into(), LExpr::Nil),
                ]),
            ));
        }
        for (name, _) in trait_methods {
            if name.starts_with("__") || !seen.insert(name.clone()) {
                continue;
            }
            entries.push(LField::Named(
                name.clone(),
                LExpr::Table(vec![
                    LField::Named("name".into(), LExpr::Str(format!("{:?}", name))),
                    LField::Named("kind".into(), LExpr::Str("\"method\"".into())),
                    LField::Named("isconst".into(), LExpr::Bool(false)),
                    LField::Named("visibility".into(), LExpr::Str("\"public\"".into())),
                    LField::Named("position".into(), LExpr::Nil),
                ]),
            ));
        }
        LStmt::Assign {
            targets: vec![LExpr::field(LExpr::ident(class.name.clone()), "members")],
            op: None,
            exprs: vec![LExpr::Table(entries)],
        }
    }

    fn traits_metadata(&mut self, class: &ClassDecl) -> LStmt {
        let mut names = HashSet::new();
        for imp in &class.implements {
            collect_trait_closure(&imp.name, &self.traits, &mut names);
        }
        let mut entries = Vec::new();
        for n in names {
            entries.push(LField::Named(n, LExpr::Bool(true)));
        }
        LStmt::Assign {
            targets: vec![LExpr::field(LExpr::ident(class.name.clone()), "__traits")],
            op: None,
            exprs: vec![LExpr::Table(entries)],
        }
    }

    fn lower_trait(&mut self, trait_decl: &TraitDecl) -> Result<Vec<LStmt>, String> {
        let info = TraitInfo::from(trait_decl);
        let methods = info.methods();
        let fields = info.fields();

        if !trait_decl.params.is_empty() {
            let mut body = Vec::new();
            for p in &trait_decl.params {
                if let Some(d) = &p.default {
                    body.push(LStmt::If {
                        cond: LExpr::Binary {
                            op: "==".into(),
                            left: Box::new(LExpr::ident(p.name.clone())),
                            right: Box::new(LExpr::Nil),
                        },
                        then_body: vec![LStmt::Assign {
                            targets: vec![LExpr::ident(p.name.clone())],
                            op: None,
                            exprs: vec![self.lower_expr(d)?],
                        }],
                        else_body: None,
                    });
                }
            }
            let mut table_fields = Vec::new();
            for p in &trait_decl.params {
                table_fields.push(LField::Named(
                    p.name.clone(),
                    LExpr::ident(p.name.clone()),
                ));
            }
            for f in &fields {
                if let Some(d) = &f.default {
                    table_fields.push(LField::Named(f.name.clone(), self.lower_expr(d)?));
                }
            }
            body.push(LStmt::Const {
                names: vec!["trait".into()],
                exprs: vec![LExpr::Table(table_fields)],
            });
            for m in &methods {
                body.extend(self.lower_method("trait", m)?);
            }
            body.push(LStmt::Return(vec![LExpr::ident("trait")]));
            return Ok(vec![LStmt::Function {
                prefix: format!("const function {}", trait_decl.name),
                params: trait_decl
                    .params
                    .iter()
                    .filter(|p| p.name != "...")
                    .map(|p| p.name.clone())
                    .collect(),
                body,
            }]);
        }

        if info.has_create() {
            let mut out = Vec::new();
            // The trait stays a table so that its methods can be copied into
            // implementing classes; `__create` makes the table callable.
            out.push(LStmt::Const {
                names: vec![trait_decl.name.clone()],
                exprs: vec![LExpr::Table(Vec::new())],
            });
            for m in &methods {
                if m.name == "__create" || m.name == "__implemented" {
                    continue;
                }
                out.extend(self.lower_method(&trait_decl.name, m)?);
            }
            for f in &fields {
                if let Some(d) = &f.default {
                    out.push(LStmt::Assign {
                        targets: vec![LExpr::field(
                            LExpr::ident(trait_decl.name.clone()),
                            f.name.clone(),
                        )],
                        op: None,
                        exprs: vec![self.lower_expr(d)?],
                    });
                }
            }
            if let Some(create) = methods.iter().find(|m| m.name == "__create") {
                let (params, defaults) = self.lower_params(&create.params)?;
                let mut body = defaults;
                body.extend(self.with_function_scope(|this| this.lower_block(&create.body))?);
                let mut call_params = vec!["_".to_string()];
                call_params.extend(params.iter().cloned());
                out.push(LStmt::Call(LExpr::call(
                    LExpr::ident("setmetatable"),
                    vec![
                        LExpr::ident(trait_decl.name.clone()),
                        LExpr::Table(vec![LField::Named(
                            "__call".into(),
                            LExpr::Function {
                                params: call_params,
                                body,
                            },
                        )]),
                    ],
                )));
            }
            if let Some(implemented) = methods.iter().find(|m| m.name == "__implemented") {
                let (params, defaults) = self.lower_params(&implemented.params)?;
                let mut body = defaults;
                body.extend(
                    self.with_function_scope(|this| this.lower_implemented_body(&implemented.body))?,
                );
                out.push(LStmt::Function {
                    prefix: "const function implemented".into(),
                    params,
                    body,
                });
            }
            return Ok(out);
        }

        if methods.is_empty() && fields.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = vec![LStmt::Const {
            names: vec![trait_decl.name.clone()],
            exprs: vec![LExpr::Table(Vec::new())],
        }];
        for m in &methods {
            out.extend(self.lower_method(&trait_decl.name, m)?);
        }
        for f in &fields {
            if let Some(d) = &f.default {
                out.push(LStmt::Assign {
                    targets: vec![LExpr::field(LExpr::ident(trait_decl.name.clone()), f.name.clone())],
                    op: None,
                    exprs: vec![self.lower_expr(d)?],
                });
            }
        }
        Ok(out)
    }

    fn lower_implemented_body(&mut self, body: &[Stmt]) -> Result<Vec<LStmt>, String> {
        let mut out = Vec::new();
        for stmt in body {
            // Registry[cls] = true is the Luwu idiom for adding a class to a
            // list. The generated __create loops over the list's values.
            if let Stmt::Assign { targets, op, exprs } = stmt {
                if op.is_none() && targets.len() == 1 && exprs.len() == 1 {
                    if let (Expr::Index(base, key), Expr::Bool(true)) =
                        (&targets[0], &exprs[0])
                    {
                        if let (Expr::Ident(reg), Expr::Ident(key_name)) =
                            (base.as_ref(), key.as_ref())
                        {
                            out.push(LStmt::Call(LExpr::call(
                                LExpr::field(LExpr::ident("table"), "insert"),
                                vec![
                                    LExpr::ident(reg.clone()),
                                    LExpr::ident(key_name.clone()),
                                ],
                            )));
                            continue;
                        }
                    }
                }
            }
            out.extend(self.lower_stmt(stmt)?);
        }
        Ok(out)
    }
}

struct TraitInfo<'a> {
    decl: &'a TraitDecl,
}

impl<'a> TraitInfo<'a> {
    fn from(decl: &'a TraitDecl) -> Self {
        Self { decl }
    }

    fn methods(&self) -> Vec<&'a MethodDecl> {
        self.decl
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(m) if !m.is_expect => Some(m),
                _ => None,
            })
            .collect()
    }

    fn fields(&self) -> Vec<&'a FieldDecl> {
        self.decl
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Field(f) if !f.is_expect => Some(f),
                _ => None,
            })
            .collect()
    }

    fn copyable_methods(&self) -> Vec<&'a MethodDecl> {
        // Only instance methods are copied into implementing classes: static
        // methods stay on the trait and are called as `Trait.name(...)`, and
        // `final` instance methods are still the implementation to inherit.
        self.methods()
            .into_iter()
            .filter(|m| {
                !m.is_static
                    && m.params.first().is_some_and(|p| p.name == "self")
                    && m.name != "__create"
                    && m.name != "__implemented"
            })
            .collect()
    }

    fn instance_fields(&self) -> Vec<&'a FieldDecl> {
        // Parameterized traits expose their parameters as instance fields.
        let mut out = Vec::new();
        for p in &self.decl.params {
            // There is no FieldDecl for params; the caller maps params from the
            // factory table directly, so this remains empty here.
            let _ = p;
        }
        out.extend(self.fields());
        out
    }

    fn has_create(&self) -> bool {
        self.methods().iter().any(|m| m.name == "__create")
    }

    fn has_implemented(&self) -> bool {
        self.methods().iter().any(|m| m.name == "__implemented")
    }
}

fn collect_traits(stmts: &[Stmt], traits: &mut HashMap<String, TraitDecl>) {
    for stmt in stmts {
        match stmt {
            Stmt::Trait(t) => {
                traits.insert(t.name.clone(), t.clone());
            }
            Stmt::Function { body, .. } => collect_traits(body, traits),
            Stmt::If {
                branches,
                else_body,
            } => {
                for (_, b) in branches {
                    collect_traits(b, traits);
                }
                if let Some(b) = else_body {
                    collect_traits(b, traits);
                }
            }
            Stmt::IfBind {
                then_body,
                else_body,
                ..
            } => {
                collect_traits(then_body, traits);
                if let Some(b) = else_body {
                    collect_traits(b, traits);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::Do { body } => {
                collect_traits(body, traits)
            }
            _ => {}
        }
    }
}

fn scan_stmts(stmts: &[Stmt], usage: &mut Usage) {
    for stmt in stmts {
        match stmt {
            Stmt::Decl { exprs, .. } => {
                for e in exprs {
                    scan_expr(e, usage);
                }
            }
            Stmt::Function { body, .. } => scan_stmts(body, usage),
            Stmt::Assign { targets, exprs, .. } => {
                for e in targets {
                    scan_expr(e, usage);
                }
                for e in exprs {
                    scan_expr(e, usage);
                }
            }
            Stmt::Call(e) => scan_expr(e, usage),
            Stmt::Return(exprs) => {
                for e in exprs {
                    scan_expr(e, usage);
                }
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (c, b) in branches {
                    scan_expr(c, usage);
                    scan_stmts(b, usage);
                }
                if let Some(b) = else_body {
                    scan_stmts(b, usage);
                }
            }
            Stmt::IfBind {
                expr,
                when_cond,
                then_body,
                else_body,
                ..
            } => {
                scan_expr(expr, usage);
                if let Some(c) = when_cond {
                    scan_expr(c, usage);
                }
                scan_stmts(then_body, usage);
                if let Some(b) = else_body {
                    scan_stmts(b, usage);
                }
            }
            Stmt::For {
                iter,
                when_clause,
                body,
                ..
            } => {
                scan_expr(iter, usage);
                if let Some(WhenClause::Cond(c)) = when_clause {
                    scan_expr(c, usage);
                }
                if let Some(WhenClause::Bind { expr, .. }) = when_clause {
                    scan_expr(expr, usage);
                }
                scan_stmts(body, usage);
            }
            Stmt::While { cond, body } => {
                scan_expr(cond, usage);
                scan_stmts(body, usage);
            }
            Stmt::Repeat { body, cond } => {
                scan_stmts(body, usage);
                scan_expr(cond, usage);
            }
            Stmt::Do { body } => scan_stmts(body, usage),
            Stmt::Give(e) => scan_expr(e, usage),
            Stmt::Class(c) => {
                for imp in &c.implements {
                    for a in &imp.args {
                        scan_expr(a, usage);
                    }
                }
                for m in &c.members {
                    if let ClassMember::Method(m) = m {
                        scan_stmts(&m.body, usage);
                    }
                    if let ClassMember::Field(f) = m {
                        if let Some(d) = &f.default {
                            scan_expr(d, usage);
                        }
                    }
                }
            }
            Stmt::Trait(t) => {
                for m in &t.members {
                    if let ClassMember::Method(m) = m {
                        scan_stmts(&m.body, usage);
                    }
                    if let ClassMember::Field(f) = m {
                        if let Some(d) = &f.default {
                            scan_expr(d, usage);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn scan_expr(expr: &Expr, usage: &mut Usage) {
    match expr {
        Expr::Call { func, args, .. } => {
            if let Expr::Field(base, name) = func.as_ref() {
                if let Expr::Ident(ns) = base.as_ref() {
                    if ns == "class" {
                        match name.as_str() {
                            "fields" => usage.fields = true,
                            "member" => usage.member = true,
                            "name" => usage.name = true,
                            "of" => usage.of = true,
                            "isinstance" => usage.isinstance = true,
                            "implements" => usage.implements = true,
                            _ => {}
                        }
                    }
                }
            }
            scan_expr(func, usage);
            for a in args {
                scan_expr(a, usage);
            }
        }
        Expr::Table(fields) => {
            for f in fields {
                match f {
                    TableField::Named(_, e) | TableField::Positional(e) => scan_expr(e, usage),
                    TableField::Computed(k, v) => {
                        scan_expr(k, usage);
                        scan_expr(v, usage)
                    }
                }
            }
        }
        Expr::Bundle(items) => {
            for i in items {
                scan_expr(i, usage);
            }
        }
        Expr::Index(a, b) => {
            scan_expr(a, usage);
            scan_expr(b, usage);
        }
        Expr::Field(a, _) => scan_expr(a, usage),
        Expr::Unary { expr, .. } => scan_expr(expr, usage),
        Expr::Paren(expr) => scan_expr(expr, usage),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, usage);
            scan_expr(right, usage);
        }
        Expr::OrReturn { value, fallback } => {
            scan_expr(value, usage);
            scan_expr(fallback, usage);
        }
        Expr::Try(e) => scan_expr(e, usage),
        Expr::DoBlock { body, give } => {
            scan_stmts(body, usage);
            scan_expr(give, usage);
        }
        Expr::TableComp {
            iter,
            when_cond,
            give,
            ..
        } => {
            scan_expr(iter, usage);
            if let Some(c) = when_cond {
                scan_expr(c, usage);
            }
            scan_expr(give, usage);
        }
        Expr::Is { value, .. } => scan_expr(value, usage),
        Expr::Function { body, .. } => scan_stmts(body, usage),
        _ => {}
    }
}

fn collect_none_returning(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Function { name, body, .. } => {
                if body_returns_none(body) {
                    out.insert(name.clone());
                }
                collect_none_returning(body, out);
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (_, b) in branches {
                    collect_none_returning(b, out);
                }
                if let Some(b) = else_body {
                    collect_none_returning(b, out);
                }
            }
            Stmt::IfBind {
                then_body,
                else_body,
                ..
            } => {
                collect_none_returning(then_body, out);
                if let Some(b) = else_body {
                    collect_none_returning(b, out);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::Do { body } => {
                collect_none_returning(body, out)
            }
            Stmt::Class(c) => {
                for m in &c.members {
                    if let ClassMember::Method(m) = m {
                        collect_none_returning(&m.body, out);
                    }
                }
            }
            Stmt::Trait(t) => {
                for m in &t.members {
                    if let ClassMember::Method(m) = m {
                        collect_none_returning(&m.body, out);
                    }
                }
            }
            _ => {}
        }
    }
}

fn body_returns_none(body: &[Stmt]) -> bool {
    body.iter().any(|s| match s {
        Stmt::Return(exprs) => exprs.iter().any(|e| matches!(e, Expr::None)),
        Stmt::If {
            branches,
            else_body,
        } => {
            branches.iter().any(|(_, b)| body_returns_none(b))
                || else_body.as_ref().is_some_and(|b| body_returns_none(b))
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Do { body } => {
            body_returns_none(body)
        }
        _ => false,
    })
}

fn is_iterator_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call { func, .. } => match func.as_ref() {
            Expr::Ident(n) => matches!(n.as_str(), "pairs" | "ipairs" | "next"),
            Expr::Field(_, n) => matches!(n.as_str(), "pairs" | "ipairs" | "next"),
            _ => false,
        },
        _ => false,
    }
}

fn trait_closure_names(name: &str, traits: &HashMap<String, TraitDecl>) -> Vec<String> {
    fn walk(name: &str, traits: &HashMap<String, TraitDecl>, out: &mut Vec<String>) {
        if out.iter().any(|n| n == name) {
            return;
        }
        out.push(name.to_string());
        if let Some(t) = traits.get(name) {
            for n in &t.needs {
                walk(&n.name, traits, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(name, traits, &mut out);
    out
}

fn collect_trait_closure(
    name: &str,
    traits: &HashMap<String, TraitDecl>,
    out: &mut HashSet<String>,
) {
    if !out.insert(name.to_string()) {
        return;
    }
    if let Some(t) = traits.get(name) {
        for n in &t.needs {
            collect_trait_closure(&n.name, traits, out);
        }
    }
}

fn helper_source(name: &str) -> String {
    match name {
        "isinstance" => "local function __luwu_isinstance(value, class)\n    return type(value) == \"table\" and getmetatable(value) == class\nend".into(),
        "of" => "local function __luwu_class_of(value)\n    if type(value) ~= \"table\" then\n        return nil\n    end\n    return getmetatable(value)\nend".into(),
        "name" => "local function __luwu_class_name(value)\n    if type(value) ~= \"table\" then\n        return nil\n    end\n    local cls = getmetatable(value)\n    if cls and cls.__name then\n        return cls.__name\n    end\n    return value.__name\nend".into(),
        "fields" => "local function __luwu_fields(object)\n    local cls = type(object) == \"table\" and getmetatable(object) or nil\n    if type(object) == \"table\" and rawget(object, \"fields\") then\n        cls = object\n    elseif not (cls and cls.fields) then\n        cls = object\n    end\n    local fields = cls and cls.fields or {}\n    local result = {}\n    for i, field in ipairs(fields) do\n        result[i] = {\n            name = field.name,\n            isconst = field.isconst,\n            position = field.position,\n            value = {\n                get = function()\n                    return object[field.name]\n                end,\n            },\n        }\n    end\n    return result\nend".into(),
        "member" => "local function __luwu_member(target, name)\n    local mt = type(target) == \"table\" and getmetatable(target) or nil\n    local cls\n    if type(target) == \"table\" and rawget(target, \"members\") then\n        cls = target\n    elseif mt and mt.members then\n        cls = mt\n    else\n        cls = target\n    end\n    local info = cls and cls.members and cls.members[name]\n    if not info then\n        return nil\n    end\n    local bound = cls ~= target\n    local member = {\n        name = info.name,\n        kind = info.kind,\n        isconst = info.isconst,\n        visibility = info.visibility,\n        position = info.position,\n        _target = bound and target or nil,\n        _has_value = bound,\n        _value = bound and target[name] or nil,\n    }\n    member.get = function(self, object)\n        if object then\n            return object[self.name]\n        end\n        return self._target and self._target[self.name] or nil\n    end\n    setmetatable(member, {\n        __tostring = function(self)\n            if self.kind == \"field\" then\n                if self._has_value then\n                    return string.format(\"%s const %s = %q\", self.visibility, self.name, self._value)\n                end\n                return string.format(\"%s const %s\", self.visibility, self.name)\n            end\n            return string.format(\"%s function %s\", self.visibility, self.name)\n        end,\n    })\n    return member\nend".into(),
        "implements" => "local function __luwu_implements(object, trait)\n    local cls = type(object) == \"table\" and getmetatable(object) or nil\n    return cls ~= nil and cls.__traits ~= nil and cls.__traits[trait] ~= nil\nend".into(),
        "table_drop" => "local function __luwu_table_drop(list, value)\n    local index = table.find(list, value)\n    while index ~= nil do\n        table.remove(list, index)\n        index = table.find(list, value)\n    end\nend".into(),
        _ => String::new(),
    }
}
