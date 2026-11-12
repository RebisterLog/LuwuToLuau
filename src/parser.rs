use crate::ast::*;
use crate::lexer::{Tok, Token};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

type PResult<T> = Result<T, String>;

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn cur(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn cur_token(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek(&self, n: usize) -> &Tok {
        &self.tokens[(self.pos + n).min(self.tokens.len() - 1)].tok
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn at_word(&self, w: &str) -> bool {
        self.cur().is_word(w)
    }

    fn at_sym(&self, s: &str) -> bool {
        self.cur().is_sym(s)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.at_word(w) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        if self.at_sym(s) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_word(&mut self, w: &str) -> PResult<()> {
        if self.eat_word(w) {
            Ok(())
        } else {
            Err(format!(
                "line {}:{}: expected `{w}`, found `{}`",
                self.cur_token().line,
                self.cur_token().col,
                self.cur()
            ))
        }
    }

    fn expect_sym(&mut self, s: &str) -> PResult<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(format!(
                "line {}:{}: expected `{s}`, found `{}`",
                self.cur_token().line,
                self.cur_token().col,
                self.cur()
            ))
        }
    }

    fn expect_name(&mut self) -> PResult<String> {
        if let Tok::Word(w) = self.cur().clone() {
            self.advance();
            Ok(w)
        } else {
            Err(format!(
                "line {}:{}: expected a name, found `{}`",
                self.cur_token().line,
                self.cur_token().col,
                self.cur()
            ))
        }
    }

    pub fn parse_program(&mut self) -> PResult<Program> {
        let stmts = self.parse_block_until(&[])?;
        if !matches!(self.cur(), Tok::Eof) {
            return Err(format!(
                "line {}:{}: unexpected `{}`",
                self.cur_token().line,
                self.cur_token().col,
                self.cur()
            ));
        }
        Ok(Program { stmts })
    }

    fn parse_block_until(&mut self, terms: &[&str]) -> PResult<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            while self.eat_sym(";") {}
            if matches!(self.cur(), Tok::Eof) {
                break;
            }
            if let Tok::Word(w) = self.cur() {
                if terms.iter().any(|t| t == w) {
                    break;
                }
            }
            out.push(self.parse_stmt()?);
        }
        Ok(out)
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        while self.eat_sym("@") {
            // Attributes are compile-time hints (for example `@native`); they
            // carry no runtime meaning for the generated Luau.
            let _ = self.expect_name()?;
            if self.at_sym("(") {
                self.skip_balanced("(", ")");
            }
        }
        if self.eat_word("export") {
            // `export` marks module bindings; the Luau output keeps the binding
            // itself and lets the module system decide visibility.
            // `export NAME: Type = value` is an exported const binding.
            if let Tok::Word(name) = self.peek(0) {
                if !matches!(
                    name.as_str(),
                    "const" | "local" | "function" | "type" | "class" | "trait" | "export"
                ) {
                    let name = self.expect_name()?;
                    let ty = if self.eat_sym(":") {
                        self.skip_type()
                    } else {
                        None
                    };
                    let exprs = if self.eat_sym("=") {
                        self.parse_expr_list()?
                    } else {
                        Vec::new()
                    };
                    return Ok(Stmt::Decl {
                        kind: BindingKind::Const,
                        patterns: vec![Pattern::Name(name)],
                        exprs,
                        ty,
                    });
                }
            }
            return self.parse_stmt();
        }
        if self.at_word("class") && matches!(self.peek(1), Tok::Word(_)) {
            return Ok(Stmt::Class(self.parse_class()?));
        }
        if self.at_word("trait") && matches!(self.peek(1), Tok::Word(_)) {
            return Ok(Stmt::Trait(self.parse_trait()?));
        }
        if self.at_word("const") || self.at_word("local") {
            let kind = if self.eat_word("const") {
                BindingKind::Const
            } else {
                self.expect_word("local")?;
                BindingKind::Local
            };
            if self.eat_word("function") {
                let name = self.expect_name()?;
                if self.at_sym("<") {
                    self.skip_balanced("<", ">");
                }
                let params = self.parse_params()?;
                let return_type = if self.eat_sym(":") {
                    self.skip_type()
                } else {
                    None
                };
                let body = self.parse_block_until(&["end"])?;
                self.expect_word("end")?;
                return Ok(Stmt::Function {
                    kind,
                    name,
                    params,
                    body,
                    return_type,
                });
            }

            let patterns = self.parse_pattern_list_until(&["="])?;
            let ty = if self.eat_sym(":") {
                self.skip_type()
            } else {
                None
            };
            if !self.eat_sym("=") {
                return Ok(Stmt::Decl {
                    kind,
                    patterns,
                    exprs: Vec::new(),
                    ty,
                });
            }
            let exprs = self.parse_expr_list()?;
            return Ok(Stmt::Decl {
                kind,
                patterns,
                exprs,
                ty,
            });
        }
        if self.at_word("function") {
            self.advance();
            let mut name = self.expect_name()?;
            // Luau-style table/method declarations: `function M.foo()` and
            // `function M:foo()`.
            let mut target = None;
            while self.at_sym(".") || self.at_sym(":") {
                let sep = if self.eat_sym(":") {
                    ":"
                } else {
                    self.expect_sym(".")?;
                    "."
                };
                let part = self.expect_name()?;
                name.push_str(sep);
                name.push_str(&part);
                target = Some(());
            }
            if self.at_sym("<") {
                self.skip_balanced("<", ">");
            }
            let params = self.parse_params()?;
            let return_type = if self.eat_sym(":") {
                self.skip_type()
            } else {
                None
            };
            let body = self.parse_block_until(&["end"])?;
            self.expect_word("end")?;
            return Ok(Stmt::Function {
                kind: if target.is_some() {
                    BindingKind::Global
                } else {
                    BindingKind::Local
                },
                name,
                params,
                body,
                return_type,
            });
        }
        if self.at_word("if") {
            return self.parse_if();
        }
        if self.at_word("for") {
            return self.parse_for();
        }
        if self.at_word("while") {
            self.advance();
            if self.at_word("const") || self.at_word("local") {
                self.advance();
                let name = self.expect_name()?;
                self.expect_sym("=")?;
                let expr = self.parse_expr()?;
                let when_clauses = self.parse_when_clauses()?;
                self.expect_word("do")?;
                let body = self.parse_block_until(&["end"])?;
                self.expect_word("end")?;
                return Ok(Stmt::WhileBind {
                    name,
                    expr,
                    when_clauses,
                    body,
                });
            }
            let cond = self.parse_expr()?;
            self.expect_word("do")?;
            let body = self.parse_block_until(&["end"])?;
            self.expect_word("end")?;
            return Ok(Stmt::While { cond, body });
        }
        if self.at_word("repeat") {
            self.advance();
            let body = self.parse_block_until(&["until"])?;
            self.expect_word("until")?;
            let cond = self.parse_expr()?;
            return Ok(Stmt::Repeat { body, cond });
        }
        if self.at_word("do") {
            self.advance();
            let body = self.parse_block_until(&["end"])?;
            self.expect_word("end")?;
            return Ok(Stmt::Do { body });
        }
        if self.at_word("return") {
            self.advance();
            if self.at_statement_end() {
                return Ok(Stmt::Return(Vec::new()));
            }
            let exprs = self.parse_expr_list()?;
            return Ok(Stmt::Return(exprs));
        }
        if self.eat_word("break") {
            return Ok(Stmt::Break);
        }
        if self.eat_word("continue") {
            return Ok(Stmt::Continue);
        }
        if self.at_word("give") {
            self.advance();
            let value = self.parse_expr()?;
            return Ok(Stmt::Give(value));
        }
        if self.at_word("type") && matches!(self.peek(1), Tok::Word(_)) {
            // Type aliases carry no runtime meaning; consume the whole alias,
            // including multi-line table/function types.
            self.expect_word("type")?;
            let _ = self.expect_name()?;
            if self.at_sym("<") {
                self.skip_balanced("<", ">");
            }
            if self.eat_sym("=") {
                let _ = self.skip_type_decl();
            }
            return Ok(Stmt::Do { body: Vec::new() });
        }

        // Expression statement or assignment.
        let first = self.parse_expr()?;
        let mut targets = vec![first];
        while self.eat_sym(",") {
            targets.push(self.parse_expr()?);
        }
        if let Some(op) = self.parse_assign_op() {
            let exprs = self.parse_expr_list()?;
            return Ok(Stmt::Assign { targets, op, exprs });
        }
        if targets.len() > 1 {
            return Err(format!(
                "line {}:{}: expected an assignment after a list of expressions",
                self.cur_token().line,
                self.cur_token().col
            ));
        }
        let first = targets.remove(0);
        Ok(Stmt::Call(first))
    }

    fn parse_assign_op(&mut self) -> Option<Option<String>> {
        for op in ["+=", "-=", "*=", "/=", "%=", "^=", "..="] {
            if self.eat_sym(op) {
                return Some(Some(op.to_string()));
            }
        }
        if self.eat_sym("=") {
            return Some(None);
        }
        None
    }

    fn at_statement_end(&self) -> bool {
        match self.cur() {
            Tok::Eof => true,
            Tok::Sym(s) if s == ";" => true,
            Tok::Word(w) => matches!(
                w.as_str(),
                "end"
                    | "else"
                    | "elseif"
                    | "until"
                    | "when"
                    | "then"
                    | "give"
                    | "in"
                    | "implements"
                    | "needs"
            ),
            _ => false,
        }
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        self.expect_word("if")?;
        let stmt = self.parse_if_branch_after_keyword()?;
        self.expect_word("end")?;
        Ok(stmt)
    }

    /// Parses one `if`/`elseif` branch after the keyword has been consumed.
    /// Consumes the closing `end` for the whole chain.
    fn parse_if_branch_after_keyword(&mut self) -> PResult<Stmt> {
        if self.at_word("const") || self.at_word("local") {
            self.advance();
            let name = self.expect_name()?;
            self.expect_sym("=")?;
            let expr = self.parse_expr()?;
            let when_clauses = self.parse_when_clauses()?;
            self.expect_word("then")?;
            let then_body = self.parse_block_until(&["elseif", "else", "end"])?;
            let else_body = self.parse_if_else_tail()?;
            return Ok(Stmt::IfBind {
                name,
                expr,
                when_clauses,
                then_body,
                else_body,
            });
        }

        let cond = self.parse_expr()?;
        self.expect_word("then")?;
        let body = self.parse_block_until(&["elseif", "else", "end"])?;
        let else_body = self.parse_if_else_tail()?;
        Ok(Stmt::If {
            branches: vec![(cond, body)],
            else_body,
        })
    }

    fn parse_if_else_tail(&mut self) -> PResult<Option<Vec<Stmt>>> {
        if self.eat_word("elseif") {
            let nested = self.parse_if_branch_after_keyword()?;
            return Ok(Some(vec![nested]));
        }
        if self.eat_word("else") {
            return Ok(Some(self.parse_block_until(&["end"])?));
        }
        Ok(None)
    }

    fn parse_for(&mut self) -> PResult<Stmt> {
        self.expect_word("for")?;
        // Numeric loop: `for i = start, stop [, step] do`.
        if matches!(self.cur(), Tok::Word(_)) && self.peek(1).is_sym("=") {
            let name = self.expect_name()?;
            self.expect_sym("=")?;
            let start = self.parse_expr()?;
            self.expect_sym(",")?;
            let stop = self.parse_expr()?;
            let step = if self.eat_sym(",") {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let when_clauses = self.parse_when_clauses()?;
            self.expect_word("do")?;
            let body = self.parse_block_until(&["end"])?;
            self.expect_word("end")?;
            return Ok(Stmt::NumericFor {
                name,
                start,
                stop,
                step,
                when_clauses,
                body,
            });
        }
        let patterns = self.parse_pattern_list_until(&["in"])?;
        self.expect_word("in")?;
        let iter = self.parse_expr()?;
        let when_clauses = self.parse_when_clauses()?;
        self.expect_word("do")?;
        let body = self.parse_block_until(&["end"])?;
        self.expect_word("end")?;
        Ok(Stmt::For {
            patterns,
            iter,
            when_clauses,
            body,
        })
    }

    fn parse_when_clauses(&mut self) -> PResult<Vec<WhenClause>> {
        let mut clauses = Vec::new();
        while self.eat_word("when") {
            if self.at_word("const") || self.at_word("local") {
                self.advance();
                let name = self.expect_name()?;
                self.expect_sym("=")?;
                let expr = self.parse_expr()?;
                clauses.push(WhenClause::Bind { name, expr });
            } else {
                clauses.push(WhenClause::Cond(self.parse_expr()?));
            }
        }
        Ok(clauses)
    }

    fn parse_pattern_list_until(&mut self, terms: &[&str]) -> PResult<Vec<Pattern>> {
        let mut out = Vec::new();
        loop {
            if let Tok::Word(w) = self.cur() {
                if terms.iter().any(|t| t == w) {
                    break;
                }
            }
            if self.at_sym("=") || matches!(self.cur(), Tok::Eof) {
                break;
            }
            out.push(self.parse_pattern()?);
            if !self.eat_sym(",") {
                break;
            }
        }
        Ok(out)
    }

    fn parse_pattern(&mut self) -> PResult<Pattern> {
        if self.eat_sym(".") {
            if self.eat_sym("{") {
                let names = self.parse_name_list("}")?;
                return Ok(Pattern::Table(names));
            }
            if self.eat_sym("(") {
                let names = self.parse_name_list(")")?;
                return Ok(Pattern::Bundle(names));
            }
            return Err(format!(
                "line {}:{}: expected `{{` or `(` after `.` in pattern",
                self.cur_token().line,
                self.cur_token().col
            ));
        }
        Ok(Pattern::Name(self.expect_name()?))
    }

    fn parse_name_list(&mut self, close: &str) -> PResult<Vec<String>> {
        let mut names = Vec::new();
        loop {
            if self.eat_sym(close) {
                break;
            }
            names.push(self.expect_name()?);
            if self.eat_sym(close) {
                break;
            }
            self.expect_sym(",")?;
        }
        Ok(names)
    }

    fn parse_class(&mut self) -> PResult<ClassDecl> {
        self.expect_word("class")?;
        let name = self.expect_name()?;
        if self.at_sym("<") {
            self.skip_balanced("<", ">");
        }
        let class_visibility = if self.at_word("public") || self.at_word("private") {
            let v = match self.cur() {
                Tok::Word(w) => Some(w.clone()),
                _ => None,
            };
            self.advance();
            v
        } else {
            None
        };
        let (mut params, has_ctor_parens) = if self.at_sym("(") {
            (self.parse_params()?, true)
        } else {
            (Vec::new(), false)
        };
        if let Some(visibility) = class_visibility {
            for p in &mut params {
                if p.visibility.is_none() {
                    p.visibility = Some(visibility.clone());
                }
            }
        }
        let mut implements = Vec::new();
        if self.eat_word("implements") {
            loop {
                implements.push(self.parse_trait_ref()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        let mut members = Vec::new();
        while !self.at_word("end") && !matches!(self.cur(), Tok::Eof) {
            members.push(self.parse_class_member()?);
            while self.eat_sym(";") {}
        }
        self.expect_word("end")?;
        Ok(ClassDecl {
            name,
            params,
            has_ctor_parens,
            implements,
            members,
        })
    }

    fn parse_trait(&mut self) -> PResult<TraitDecl> {
        self.expect_word("trait")?;
        let name = self.expect_name()?;
        if self.at_sym("<") {
            self.skip_balanced("<", ">");
        }
        let params = if self.at_sym("(") {
            self.parse_params()?
        } else {
            Vec::new()
        };
        let mut needs = Vec::new();
        if self.eat_word("needs") {
            loop {
                needs.push(self.parse_trait_ref()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        let mut members = Vec::new();
        while !self.at_word("end") && !matches!(self.cur(), Tok::Eof) {
            members.push(self.parse_class_member()?);
            while self.eat_sym(";") {}
        }
        self.expect_word("end")?;
        Ok(TraitDecl {
            name,
            params,
            needs,
            members,
        })
    }

    fn parse_trait_ref(&mut self) -> PResult<TraitRef> {
        let name = self.expect_name()?;
        if self.at_sym("<") {
            self.skip_balanced("<", ">");
        }
        let args = if self.eat_sym("(") {
            self.parse_expr_list_until(")")?
        } else {
            Vec::new()
        };
        Ok(TraitRef { name, args })
    }

    fn parse_class_member(&mut self) -> PResult<ClassMember> {
        while self.eat_sym("@") {
            let _ = self.expect_name()?;
            if self.at_sym("(") {
                self.skip_balanced("(", ")");
            }
        }
        let mut mods: Vec<String> = Vec::new();
        loop {
            if let Tok::Word(w) = self.cur() {
                if matches!(
                    w.as_str(),
                    "public" | "private" | "const" | "final" | "static" | "expect"
                ) {
                    mods.push(w.clone());
                    self.advance();
                    continue;
                }
            }
            if self.eat_sym("@") {
                let _ = self.expect_name()?;
                if self.at_sym("(") {
                    self.skip_balanced("(", ")");
                }
                continue;
            }
            break;
        }
        let is_expect = mods.iter().any(|m| m == "expect");
        if self.eat_word("function") {
            let name = self.expect_name()?;
            if self.at_sym("<") {
                self.skip_balanced("<", ">");
            }
            let params = self.parse_params()?;
            let return_type = if self.eat_sym(":") {
                self.skip_type()
            } else {
                None
            };
            if is_expect {
                let has_self = params.first().is_some_and(|p| p.name == "self");
                let is_static = mods.iter().any(|m| m == "static") || !has_self;
                return Ok(ClassMember::Method(MethodDecl {
                    name,
                    params,
                    body: Vec::new(),
                    visibility: mods
                        .iter()
                        .find(|m| *m == "public" || *m == "private")
                        .cloned(),
                    is_final: false,
                    is_static,
                    is_expect: true,
                    return_type,
                }));
            }
            let body = self.parse_block_until(&["end"])?;
            self.expect_word("end")?;
            return Ok(ClassMember::Method(MethodDecl {
                name,
                params,
                body,
                visibility: mods
                    .iter()
                    .find(|m| *m == "public" || *m == "private")
                    .cloned(),
                is_final: mods.iter().any(|m| m == "final"),
                is_static: mods.iter().any(|m| m == "static"),
                is_expect: false,
                return_type,
            }));
        }

        let name = self.expect_name()?;
        let ty = if self.eat_sym(":") {
            self.skip_type()
        } else {
            None
        };
        let default = if self.eat_sym("=") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(ClassMember::Field(FieldDecl {
            name,
            default,
            visibility: mods
                .iter()
                .find(|m| *m == "public" || *m == "private")
                .cloned(),
            is_const: mods.iter().any(|m| m == "const"),
            is_expect,
            is_static: mods.iter().any(|m| m == "static"),
            ty,
        }))
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.expect_sym("(")?;
        let mut params = Vec::new();
        loop {
            if self.eat_sym(")") {
                break;
            }
            let mut mods: Vec<String> = Vec::new();
            loop {
                if let Tok::Word(w) = self.cur() {
                    if matches!(
                        w.as_str(),
                        "public" | "private" | "const" | "final" | "static" | "expect"
                    ) {
                        mods.push(w.clone());
                        self.advance();
                        continue;
                    }
                }
                break;
            }
            let name = if self.eat_sym("...") {
                "...".to_string()
            } else {
                self.expect_name()?
            };
            let ty = if self.eat_sym(":") {
                self.skip_type()
            } else {
                None
            };
            let default = if self.eat_sym("=") {
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(Param {
                name,
                default,
                visibility: mods
                    .iter()
                    .find(|m| *m == "public" || *m == "private")
                    .cloned(),
                is_const: mods.iter().any(|m| *m == "const"),
                ty,
            });
            if self.eat_sym(")") {
                break;
            }
            self.expect_sym(",")?;
        }
        Ok(params)
    }

    /// Consume a type annotation and return its textual form. Types are
    /// erased from the generated Luau, but kept for validation.
    fn skip_type(&mut self) -> Option<String> {
        let start_line = self.cur_token().line;
        let mut depth: i32 = 0;
        let mut parts: Vec<String> = Vec::new();
        while !matches!(self.cur(), Tok::Eof) {
            if depth == 0 {
                if self.cur_token().line > start_line {
                    break;
                }
                match self.cur() {
                    Tok::Sym(s) if s == "=" || s == "," || s == ")" => break,
                    Tok::Word(w)
                        if matches!(
                            w.as_str(),
                            "then"
                                | "do"
                                | "end"
                                | "return"
                                | "function"
                                | "local"
                                | "const"
                                | "if"
                                | "for"
                                | "while"
                                | "public"
                                | "private"
                                | "final"
                                | "expect"
                                | "needs"
                                | "implements"
                                | "give"
                                | "when"
                                | "else"
                                | "elseif"
                                | "until"
                        ) =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
            match self.cur().clone() {
                Tok::Sym(s) if s == "(" || s == "{" || s == "[" || s == "<" => {
                    parts.push(s);
                    depth += 1;
                    self.advance();
                }
                Tok::Sym(s) if s == "<<" => {
                    parts.push(s);
                    depth += 2;
                    self.advance();
                }
                Tok::Sym(s) if s == ")" || s == "}" || s == "]" || s == ">" => {
                    if depth == 0 {
                        break;
                    }
                    parts.push(s);
                    depth -= 1;
                    self.advance();
                }
                Tok::Sym(s) if s == ">>" => {
                    if depth == 0 {
                        break;
                    }
                    parts.push(s);
                    depth -= 2;
                    self.advance();
                }
                tok => {
                    parts.push(tok.to_string());
                    self.advance();
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(""))
        }
    }

    fn skip_type_decl(&mut self) -> Option<String> {
        let start_line = self.cur_token().line;
        let mut depth: i32 = 0;
        let mut parts: Vec<String> = Vec::new();
        while !matches!(self.cur(), Tok::Eof) {
            if depth == 0 && self.cur_token().line > start_line {
                let starts_statement = match self.cur() {
                    Tok::Word(w) => matches!(
                        w.as_str(),
                        "class"
                            | "trait"
                            | "const"
                            | "local"
                            | "function"
                            | "type"
                            | "export"
                            | "if"
                            | "for"
                            | "while"
                            | "repeat"
                            | "do"
                            | "return"
                            | "break"
                            | "continue"
                            | "give"
                            | "end"
                            | "else"
                            | "elseif"
                            | "until"
                            | "when"
                            | "then"
                            | "implements"
                            | "needs"
                    ),
                    Tok::Sym(s) if s == "@" => true,
                    _ => false,
                };
                if starts_statement {
                    break;
                }
            }
            match self.cur().clone() {
                Tok::Sym(s) if s == "(" || s == "{" || s == "[" || s == "<" => {
                    parts.push(s);
                    depth += 1;
                    self.advance();
                }
                Tok::Sym(s) if s == "<<" => {
                    parts.push(s);
                    depth += 2;
                    self.advance();
                }
                Tok::Sym(s) if s == ")" || s == "}" || s == "]" || s == ">" => {
                    if depth == 0 {
                        break;
                    }
                    parts.push(s);
                    depth -= 1;
                    self.advance();
                }
                Tok::Sym(s) if s == ">>" => {
                    if depth == 0 {
                        break;
                    }
                    parts.push(s);
                    depth -= 2;
                    self.advance();
                }
                tok => {
                    parts.push(tok.to_string());
                    self.advance();
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(""))
        }
    }

    fn skip_balanced(&mut self, open: &str, close: &str) {
        if !self.eat_sym(open) {
            return;
        }
        let mut depth = 1i32;
        while depth > 0 && !matches!(self.cur(), Tok::Eof) {
            if close == ">" && self.at_sym(">>") {
                self.advance();
                depth -= 2;
            } else if open == "<" && self.at_sym("<<") {
                self.advance();
                depth += 2;
            } else if self.eat_sym(open) {
                depth += 1;
            } else if self.eat_sym(close) {
                depth -= 1;
            } else {
                self.advance();
            }
        }
    }

    pub fn parse_expr_list(&mut self) -> PResult<Vec<Expr>> {
        let mut out = Vec::new();
        if self.at_statement_end() || self.at_sym(")") || self.at_sym("}") || self.at_sym("]") {
            return Ok(out);
        }
        loop {
            out.push(self.parse_expr()?);
            if self.eat_sym(",") {
                continue;
            }
            break;
        }
        Ok(out)
    }

    fn parse_expr_list_until(&mut self, close: &str) -> PResult<Vec<Expr>> {
        let mut out = Vec::new();
        if self.eat_sym(close) {
            return Ok(out);
        }
        loop {
            out.push(self.parse_expr()?);
            if self.eat_sym(close) {
                break;
            }
            self.expect_sym(",")?;
        }
        Ok(out)
    }

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_expr_prec(0)
    }

    fn parse_expr_prec(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut left = self.parse_unary()?;
        while let Some((op, prec, right_assoc)) = self.binary_op() {
            if prec < min_prec {
                break;
            }
            self.advance();

            if op == "is" {
                let kind = self.expect_name()?;
                self.expect_sym(".")?;
                self.expect_sym("(")?;
                let binding = self.expect_name()?;
                self.expect_sym(")")?;
                left = Expr::Is {
                    value: Box::new(left),
                    kind,
                    binding,
                };
                continue;
            }

            if op == "or" && self.at_word("return") {
                self.advance();
                let fallback = if self.at_statement_end()
                    || self.at_sym(")")
                    || self.at_sym("}")
                    || self.at_sym("]")
                    || self.at_sym(",")
                {
                    None
                } else {
                    Some(Box::new(self.parse_expr_prec(0)?))
                };
                left = Expr::OrReturn {
                    value: Box::new(left),
                    fallback,
                };
                continue;
            }

            if op == "or" && (self.at_word("break") || self.at_word("continue")) {
                let action = if self.eat_word("break") {
                    FlowAction::Break
                } else {
                    self.expect_word("continue")?;
                    FlowAction::Continue
                };
                left = Expr::OrFlow {
                    value: Box::new(left),
                    action,
                };
                continue;
            }

            if (op == "and" || op == "or") && (self.at_word("break") || self.at_word("continue")) {
                let action = if self.eat_word("break") {
                    FlowAction::Break
                } else {
                    self.expect_word("continue")?;
                    FlowAction::Continue
                };
                left = Expr::Guard {
                    value: Box::new(left),
                    action,
                    when_truthy: op == "and",
                };
                continue;
            }

            let next_min = if right_assoc { prec } else { prec + 1 };
            let right = self.parse_expr_prec(next_min)?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        while self.eat_sym("::") {
            // Type assertions are compile-time only.
            self.skip_cast_type();
        }
        Ok(left)
    }

    /// Consumes a type annotation that must stay on the current line (used by
    /// `value :: Type` casts so following statements are not swallowed).
    fn skip_cast_type(&mut self) {
        let line = self.cur_token().line;
        let mut depth: i32 = 0;
        while !matches!(self.cur(), Tok::Eof) {
            if depth == 0 && self.cur_token().line > line {
                break;
            }
            match self.cur().clone() {
                Tok::Sym(s) if s == "(" || s == "{" || s == "[" || s == "<" => {
                    depth += 1;
                    self.advance();
                }
                Tok::Sym(s) if s == "<<" => {
                    depth += 2;
                    self.advance();
                }
                Tok::Sym(s) if s == ")" || s == "}" || s == "]" || s == ">" => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    self.advance();
                }
                Tok::Sym(s) if s == ">>" => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 2;
                    self.advance();
                }
                Tok::Sym(s) if depth == 0 && (s == "," || s == ";") => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn binary_op(&self) -> Option<(String, u8, bool)> {
        let (op, prec, right) = match self.cur() {
            Tok::Sym(s) if s == ".." => (s.clone(), 8, true),
            Tok::Sym(s) if matches!(s.as_str(), "==" | "~=" | "<" | ">" | "<=" | ">=") => {
                (s.clone(), 3, false)
            }
            Tok::Sym(s) if s == "|" => (s.clone(), 4, false),
            Tok::Sym(s) if s == "~" => (s.clone(), 5, false),
            Tok::Sym(s) if s == "&" => (s.clone(), 6, false),
            Tok::Sym(s) if s == "<<" || s == ">>" => (s.clone(), 7, false),
            Tok::Sym(s) if s == "+" || s == "-" => (s.clone(), 9, false),
            Tok::Sym(s) if s == "*" || s == "/" || s == "//" || s == "%" => (s.clone(), 10, false),
            Tok::Sym(s) if s == "^" => (s.clone(), 12, true),
            Tok::Word(w) if w == "or" => (w.clone(), 1, false),
            Tok::Word(w) if w == "and" => (w.clone(), 2, false),
            Tok::Word(w) if w == "is" => (w.clone(), 3, false),
            _ => return None,
        };
        Some((op, prec, right))
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        if self.eat_sym("-") {
            return Ok(Expr::Unary {
                op: "-".into(),
                expr: Box::new(self.parse_expr_prec(11)?),
            });
        }
        if self.eat_sym("#") {
            return Ok(Expr::Unary {
                op: "#".into(),
                expr: Box::new(self.parse_expr_prec(11)?),
            });
        }
        if self.eat_sym("~") {
            return Ok(Expr::Unary {
                op: "~".into(),
                expr: Box::new(self.parse_expr_prec(11)?),
            });
        }
        if self.eat_word("not") {
            return Ok(Expr::Unary {
                op: "not".into(),
                expr: Box::new(self.parse_expr_prec(11)?),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            // Explicit type instantiation: `f::<T>(args)`.
            if self.at_sym("::") && self.peek(1).is_sym("<") {
                self.advance();
                self.skip_balanced("<", ">");
                continue;
            }
            if self.eat_sym(".") {
                if let Tok::Number(n) = self.cur().clone() {
                    self.advance();
                    expr = Expr::Index(Box::new(expr), Box::new(Expr::Number(n)));
                    continue;
                }
                let name = self.expect_name()?;
                expr = Expr::Field(Box::new(expr), name);
                continue;
            }
            if self.eat_sym("[") {
                let idx = self.parse_expr()?;
                self.expect_sym("]")?;
                expr = Expr::Index(Box::new(expr), Box::new(idx));
                continue;
            }
            if self.eat_sym(":") {
                let name = self.expect_name()?;
                let args = if self.at_sym("(") {
                    self.parse_call_args()?
                } else {
                    Vec::new()
                };
                expr = Expr::Call {
                    func: Box::new(Expr::Field(Box::new(expr), name)),
                    args,
                    method: true,
                };
                continue;
            }
            if self.at_sym("(") {
                let args = self.parse_call_args()?;
                expr = Expr::Call {
                    func: Box::new(expr),
                    args,
                    method: false,
                };
                continue;
            }
            // Luwu allows the table-argument call sugar: Class { ... }.
            if self.at_sym("{") {
                let table = self.parse_table()?;
                expr = Expr::Call {
                    func: Box::new(expr),
                    args: vec![table],
                    method: false,
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> PResult<Vec<Expr>> {
        self.expect_sym("(")?;
        let args = self.parse_expr_list_until(")")?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        match self.cur().clone() {
            Tok::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Tok::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Tok::Backtick(s) => {
                self.advance();
                Ok(Expr::Backtick(s))
            }
            Tok::Sym(s) if s == "..." => {
                self.advance();
                Ok(Expr::Vararg)
            }
            Tok::Sym(s) if s == "(" => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect_sym(")")?;
                Ok(Expr::Paren(Box::new(e)))
            }
            Tok::Sym(s) if s == "." => {
                self.advance();
                if self.eat_sym("(") {
                    let items = self.parse_expr_list_until(")")?;
                    Ok(Expr::Bundle(items))
                } else if self.eat_sym("{") {
                    let names = self.parse_name_list("}")?;
                    Ok(Expr::Restructure(names))
                } else {
                    Err(format!(
                        "line {}:{}: expected `(` or `{{` after `.`",
                        self.cur_token().line,
                        self.cur_token().col
                    ))
                }
            }
            Tok::Sym(s) if s == "{" => self.parse_table(),
            Tok::Word(w) => {
                self.advance();
                match w.as_str() {
                    "nil" => Ok(Expr::Nil),
                    "true" => Ok(Expr::Bool(true)),
                    "false" => Ok(Expr::Bool(false)),
                    "none" => Ok(Expr::None),
                    "if" => self.parse_if_expr(),
                    "try" => Ok(Expr::Try(Box::new(self.parse_expr_prec(11)?))),
                    "do" => {
                        // `do ... give value` ends at `give`; Luwu does not
                        // require an extra `end` after the final value.
                        let mut body = Vec::new();
                        let mut give = None;
                        loop {
                            while self.eat_sym(";") {}
                            if self.at_word("give") {
                                self.advance();
                                give = Some(self.parse_expr()?);
                                let _ = self.eat_word("end");
                                break;
                            }
                            if self.at_word("end") {
                                self.advance();
                                break;
                            }
                            if matches!(self.cur(), Tok::Eof) {
                                break;
                            }
                            body.push(self.parse_stmt()?);
                        }
                        Ok(Expr::DoBlock {
                            body,
                            give: Box::new(give.unwrap_or(Expr::Nil)),
                        })
                    }
                    "function" => {
                        let params = self.parse_params()?;
                        let return_type = if self.eat_sym(":") {
                            self.skip_type()
                        } else {
                            None
                        };
                        let body = self.parse_block_until(&["end"])?;
                        self.expect_word("end")?;
                        Ok(Expr::Function {
                            params,
                            body,
                            return_type,
                        })
                    }
                    _ => Ok(Expr::Ident(w)),
                }
            }
            other => Err(format!(
                "line {}:{}: unexpected `{}` in expression",
                self.cur_token().line,
                self.cur_token().col,
                other
            )),
        }
    }

    fn parse_if_expr(&mut self) -> PResult<Expr> {
        if self.at_word("const") || self.at_word("local") {
            self.advance();
            let name = self.expect_name()?;
            self.expect_sym("=")?;
            let expr = self.parse_expr()?;
            let when_clauses = self.parse_when_clauses()?;
            self.expect_word("then")?;
            let then_expr = self.parse_expr()?;
            self.expect_word("else")?;
            let else_expr = self.parse_expr()?;
            return Ok(Expr::ConstConditional {
                name,
                expr: Box::new(expr),
                when_clauses,
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            });
        }
        let cond = self.parse_expr()?;
        self.expect_word("then")?;
        let value = self.parse_expr()?;
        let mut branches = vec![(cond, value)];
        while self.eat_word("elseif") {
            let c = self.parse_expr()?;
            self.expect_word("then")?;
            let v = self.parse_expr()?;
            branches.push((c, v));
        }
        self.expect_word("else")?;
        let else_expr = self.parse_expr()?;
        Ok(Expr::Conditional {
            branches,
            else_expr: Box::new(else_expr),
        })
    }

    fn parse_table(&mut self) -> PResult<Expr> {
        self.expect_sym("{")?;
        if self.at_word("for") {
            self.advance();
            let patterns = self.parse_pattern_list_until(&["in"])?;
            self.expect_word("in")?;
            let iter = self.parse_expr()?;
            let when_cond = if self.eat_word("when") {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            self.expect_word("give")?;
            let give = self.parse_expr()?;
            self.expect_sym("}")?;
            return Ok(Expr::TableComp {
                patterns,
                iter: Box::new(iter),
                when_cond,
                give: Box::new(give),
            });
        }

        let mut fields = Vec::new();
        loop {
            if self.eat_sym("}") {
                break;
            }
            if self.eat_sym(",") || self.eat_sym(";") {
                continue;
            }
            if self.eat_sym("[") {
                let key = self.parse_expr()?;
                self.expect_sym("]")?;
                self.expect_sym("=")?;
                let value = self.parse_expr()?;
                fields.push(TableField::Computed(key, value));
            } else if let Tok::Word(name) = self.cur().clone() {
                if self.peek(1).is_sym("=") {
                    self.advance();
                    self.advance();
                    let value = self.parse_expr()?;
                    fields.push(TableField::Named(name, value));
                } else {
                    fields.push(TableField::Positional(self.parse_expr()?));
                }
            } else {
                fields.push(TableField::Positional(self.parse_expr()?));
            }
            if self.eat_sym(",") || self.eat_sym(";") {
                continue;
            }
            self.expect_sym("}")?;
            break;
        }
        Ok(Expr::Table(fields))
    }
}
