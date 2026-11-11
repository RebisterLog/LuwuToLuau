#[derive(Clone, Debug)]
pub enum LExpr {
    Ident(String),
    Number(String),
    Str(String),
    Backtick(String),
    Bool(bool),
    Nil,
    Paren(Box<LExpr>),
    Vararg,
    Table(Vec<LField>),
    Index(Box<LExpr>, Box<LExpr>),
    Field(Box<LExpr>, String),
    Call {
        func: Box<LExpr>,
        args: Vec<LExpr>,
        method: bool,
    },
    Binary {
        op: String,
        left: Box<LExpr>,
        right: Box<LExpr>,
    },
    Unary {
        op: String,
        expr: Box<LExpr>,
    },
    Function {
        params: Vec<String>,
        body: Vec<LStmt>,
    },
    Iife {
        params: Vec<String>,
        body: Vec<LStmt>,
        args: Vec<LExpr>,
    },
}

#[derive(Clone, Debug)]
pub enum LField {
    Positional(LExpr),
    Named(String, LExpr),
    Computed(LExpr, LExpr),
}

#[derive(Clone, Debug)]
pub enum LStmt {
    Raw(String),
    Const {
        names: Vec<String>,
        exprs: Vec<LExpr>,
    },
    Local {
        names: Vec<String>,
        exprs: Vec<LExpr>,
    },
    Assign {
        targets: Vec<LExpr>,
        op: Option<String>,
        exprs: Vec<LExpr>,
    },
    Function {
        prefix: String,
        params: Vec<String>,
        body: Vec<LStmt>,
    },
    Call(LExpr),
    Return(Vec<LExpr>),
    If {
        cond: LExpr,
        then_body: Vec<LStmt>,
        else_body: Option<Vec<LStmt>>,
    },
    While {
        cond: LExpr,
        body: Vec<LStmt>,
    },
    Repeat {
        body: Vec<LStmt>,
        cond: LExpr,
    },
    For {
        vars: Vec<String>,
        iter: LExpr,
        body: Vec<LStmt>,
    },
    Do {
        body: Vec<LStmt>,
    },
    Break,
    Continue,
}

impl LStmt {
    pub fn raw(s: impl Into<String>) -> Self {
        LStmt::Raw(s.into())
    }
}

impl LExpr {
    pub fn ident(s: impl Into<String>) -> Self {
        LExpr::Ident(s.into())
    }

    pub fn call(func: LExpr, args: Vec<LExpr>) -> Self {
        LExpr::Call {
            func: Box::new(func),
            args,
            method: false,
        }
    }

    pub fn field(base: LExpr, name: impl Into<String>) -> Self {
        LExpr::Field(Box::new(base), name.into())
    }

    pub fn index(base: LExpr, idx: LExpr) -> Self {
        LExpr::Index(Box::new(base), Box::new(idx))
    }
}

pub fn render_program(stmts: &[LStmt]) -> String {
    let mut out = String::new();
    for stmt in stmts {
        render_stmt(stmt, 0, &mut out);
    }
    out
}

fn pad(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn render_block(body: &[LStmt], indent: usize, out: &mut String) {
    for stmt in body {
        render_stmt(stmt, indent, out);
    }
}

fn render_stmt(stmt: &LStmt, indent: usize, out: &mut String) {
    match stmt {
        LStmt::Raw(s) => {
            for line in s.lines() {
                pad(indent, out);
                out.push_str(line);
                out.push('\n');
            }
        }
        LStmt::Const { names, exprs } => {
            pad(indent, out);
            out.push_str("const ");
            out.push_str(&names.join(", "));
            out.push_str(" = ");
            render_expr_list(exprs, indent, out);
            out.push('\n');
        }
        LStmt::Local { names, exprs } => {
            pad(indent, out);
            out.push_str("local ");
            out.push_str(&names.join(", "));
            out.push_str(" = ");
            render_expr_list(exprs, indent, out);
            out.push('\n');
        }
        LStmt::Assign { targets, op, exprs } => {
            pad(indent, out);
            for (i, target) in targets.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_expr(target, indent, out);
            }
            out.push(' ');
            out.push_str(op.as_deref().unwrap_or("="));
            out.push(' ');
            render_expr_list(exprs, indent, out);
            out.push('\n');
        }
        LStmt::Function { prefix, params, body } => {
            pad(indent, out);
            out.push_str(prefix);
            out.push('(');
            out.push_str(&params.join(", "));
            out.push_str(")\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("end\n");
        }
        LStmt::Call(e) => {
            pad(indent, out);
            render_expr(e, indent, out);
            out.push('\n');
        }
        LStmt::Return(exprs) => {
            pad(indent, out);
            out.push_str("return");
            if !exprs.is_empty() {
                out.push(' ');
                render_expr_list(exprs, indent, out);
            }
            out.push('\n');
        }
        LStmt::If {
            cond,
            then_body,
            else_body,
        } => {
            pad(indent, out);
            out.push_str("if ");
            render_expr(cond, indent, out);
            out.push_str(" then\n");
            render_block(then_body, indent + 1, out);
            if let Some(else_body) = else_body {
                pad(indent, out);
                out.push_str("else\n");
                render_block(else_body, indent + 1, out);
            }
            pad(indent, out);
            out.push_str("end\n");
        }
        LStmt::While { cond, body } => {
            pad(indent, out);
            out.push_str("while ");
            render_expr(cond, indent, out);
            out.push_str(" do\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("end\n");
        }
        LStmt::Repeat { body, cond } => {
            pad(indent, out);
            out.push_str("repeat\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("until ");
            render_expr(cond, indent, out);
            out.push('\n');
        }
        LStmt::For { vars, iter, body } => {
            pad(indent, out);
            out.push_str("for ");
            out.push_str(&vars.join(", "));
            out.push_str(" in ");
            render_expr(iter, indent, out);
            out.push_str(" do\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("end\n");
        }
        LStmt::Do { body } => {
            pad(indent, out);
            out.push_str("do\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("end\n");
        }
        LStmt::Break => {
            pad(indent, out);
            out.push_str("break\n");
        }
        LStmt::Continue => {
            pad(indent, out);
            out.push_str("continue\n");
        }
    }
}

fn render_expr_list(exprs: &[LExpr], indent: usize, out: &mut String) {
    for (i, e) in exprs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        render_expr(e, indent, out);
    }
}

fn render_expr(e: &LExpr, indent: usize, out: &mut String) {
    match e {
        LExpr::Number(s) | LExpr::Str(s) | LExpr::Backtick(s) => out.push_str(s),
        LExpr::Ident(s) => out.push_str(s),
        LExpr::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        LExpr::Nil => out.push_str("nil"),
        LExpr::Paren(e) => {
            out.push('(');
            render_expr(e, indent, out);
            out.push(')');
        }
        LExpr::Vararg => out.push_str("..."),
        LExpr::Table(fields) => render_table(fields, indent, out),
        LExpr::Index(base, idx) => {
            render_base(base, indent, out);
            out.push('[');
            render_expr(idx, indent, out);
            out.push(']');
        }
        LExpr::Field(base, name) => {
            render_base(base, indent, out);
            out.push('.');
            out.push_str(name);
        }
        LExpr::Call { func, args, method } => {
            if *method {
                if let LExpr::Field(base, name) = func.as_ref() {
                    render_base(base, indent, out);
                    out.push(':');
                    out.push_str(name);
                } else {
                    render_base(func, indent, out);
                }
            } else {
                render_base(func, indent, out);
            }
            out.push('(');
            render_expr_list(args, indent, out);
            out.push(')');
        }
        LExpr::Binary { op, left, right } => {
            out.push('(');
            render_expr(left, indent, out);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            render_expr(right, indent, out);
            out.push(')');
        }
        LExpr::Unary { op, expr } => {
            out.push('(');
            out.push_str(op);
            if op == "not" {
                out.push(' ');
            }
            render_expr(expr, indent, out);
            out.push(')');
        }
        LExpr::Function { params, body } => {
            out.push_str("function(");
            out.push_str(&params.join(", "));
            out.push_str(")\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("end");
        }
        LExpr::Iife { params, body, args } => {
            out.push_str("(function(");
            out.push_str(&params.join(", "));
            out.push_str(")\n");
            render_block(body, indent + 1, out);
            pad(indent, out);
            out.push_str("end)(");
            render_expr_list(args, indent, out);
            out.push(')');
        }
    }
}

fn render_base(e: &LExpr, indent: usize, out: &mut String) {
    match e {
        LExpr::Table(_) | LExpr::Function { .. } | LExpr::Iife { .. } | LExpr::Binary { .. } => {
            out.push('(');
            render_expr(e, indent, out);
            out.push(')');
        }
        LExpr::Unary { .. } => render_expr(e, indent, out),
        _ => render_expr(e, indent, out),
    }
}

fn render_table(fields: &[LField], indent: usize, out: &mut String) {
    let multiline = fields.iter().any(|f| match f {
        LField::Positional(e) | LField::Named(_, e) => contains_block_expr(e),
        LField::Computed(k, v) => contains_block_expr(k) || contains_block_expr(v),
    });
    if fields.is_empty() {
        out.push_str("{}");
        return;
    }
    if multiline {
        out.push_str("{\n");
        for f in fields {
            pad(indent + 1, out);
            render_field(f, indent + 1, out);
            out.push_str(",\n");
        }
        pad(indent, out);
        out.push('}');
    } else {
        out.push_str("{ ");
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            render_field(f, indent, out);
        }
        out.push_str(" }");
    }
}

fn render_field(f: &LField, indent: usize, out: &mut String) {
    match f {
        LField::Positional(e) => render_expr(e, indent, out),
        LField::Named(k, v) => {
            if is_ident(k) {
                out.push_str(k);
            } else {
                out.push('[');
                out.push_str(&quote(k));
                out.push(']');
            }
            out.push_str(" = ");
            render_expr(v, indent, out);
        }
        LField::Computed(k, v) => {
            out.push('[');
            render_expr(k, indent, out);
            out.push_str("] = ");
            render_expr(v, indent, out);
        }
    }
}

fn contains_block_expr(e: &LExpr) -> bool {
    match e {
        LExpr::Function { .. } | LExpr::Iife { .. } => true,
        LExpr::Table(fields) => fields.iter().any(|f| match f {
            LField::Positional(e) | LField::Named(_, e) => contains_block_expr(e),
            LField::Computed(k, v) => contains_block_expr(k) || contains_block_expr(v),
        }),
        _ => false,
    }
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn quote(s: &str) -> String {
    format!("{:?}", s)
}
