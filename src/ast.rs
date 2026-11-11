#[derive(Clone, Debug)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub enum BindingKind {
    Const,
    Local,
}

#[derive(Clone, Debug)]
pub enum Pattern {
    Name(String),
    Table(Vec<String>),
    Bundle(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
    pub visibility: Option<String>,
    pub is_const: bool,
    pub ty: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FieldDecl {
    pub name: String,
    pub default: Option<Expr>,
    pub visibility: Option<String>,
    pub is_const: bool,
    pub is_expect: bool,
    pub ty: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MethodDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub visibility: Option<String>,
    #[allow(dead_code)]
    pub is_final: bool,
    pub is_static: bool,
    pub is_expect: bool,
    pub return_type: Option<String>,
}

#[derive(Clone, Debug)]
pub enum ClassMember {
    Field(FieldDecl),
    Method(MethodDecl),
}

#[derive(Clone, Debug)]
pub struct TraitRef {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Clone, Debug)]
pub struct ClassDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub has_ctor_parens: bool,
    pub implements: Vec<TraitRef>,
    pub members: Vec<ClassMember>,
}

#[derive(Clone, Debug)]
pub struct TraitDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub needs: Vec<TraitRef>,
    pub members: Vec<ClassMember>,
}

#[derive(Clone, Debug)]
pub enum WhenClause {
    Cond(Expr),
    Bind { name: String, expr: Expr },
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Decl {
        kind: BindingKind,
        patterns: Vec<Pattern>,
        exprs: Vec<Expr>,
        ty: Option<String>,
    },
    Function {
        kind: BindingKind,
        name: String,
        params: Vec<Param>,
        body: Vec<Stmt>,
        return_type: Option<String>,
    },
    Assign {
        targets: Vec<Expr>,
        op: Option<String>,
        exprs: Vec<Expr>,
    },
    Call(Expr),
    Return(Vec<Expr>),
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
    },
    IfBind {
        name: String,
        expr: Expr,
        when_cond: Option<Expr>,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    For {
        patterns: Vec<Pattern>,
        iter: Expr,
        when_clause: Option<WhenClause>,
        body: Vec<Stmt>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    Repeat {
        body: Vec<Stmt>,
        cond: Expr,
    },
    Do {
        body: Vec<Stmt>,
    },
    Give(Expr),
    Break,
    Continue,
    Class(ClassDecl),
    Trait(TraitDecl),
}

#[derive(Clone, Debug)]
pub enum TableField {
    Named(String, Expr),
    Positional(Expr),
    Computed(Expr, Expr),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Nil,
    Bool(bool),
    Number(String),
    Str(String),
    Backtick(String),
    Ident(String),
    Paren(Box<Expr>),
    None,
    Vararg,
    Table(Vec<TableField>),
    Bundle(Vec<Expr>),
    Restructure(Vec<String>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        method: bool,
    },
    Unary {
        op: String,
        expr: Box<Expr>,
    },
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    OrReturn {
        value: Box<Expr>,
        fallback: Box<Expr>,
    },
    Try(Box<Expr>),
    DoBlock {
        body: Vec<Stmt>,
        give: Box<Expr>,
    },
    TableComp {
        patterns: Vec<Pattern>,
        iter: Box<Expr>,
        when_cond: Option<Box<Expr>>,
        give: Box<Expr>,
    },
    Is {
        value: Box<Expr>,
        kind: String,
        binding: String,
    },
    Function {
        params: Vec<Param>,
        body: Vec<Stmt>,
        return_type: Option<String>,
    },
}
