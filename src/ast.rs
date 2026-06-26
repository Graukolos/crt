#![allow(unused)]

#[derive(Debug, Clone)]
pub struct Ast {
    pub imports: Vec<Import>,
    pub item: Item,
}

#[derive(Debug, Clone)]
pub enum Item {
    Actor(Box<Actor>),
    Unit(Unit),
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: String,
    pub symbol: String,
}

#[derive(Debug, Clone)]
pub struct Actor {
    pub name: String,
    pub parameters: Vec<ActorParameter>,
    pub inports: Vec<Port>,
    pub outports: Vec<Port>,
    pub vars: Vec<VarDef>,
    pub functions: Vec<Function>,
    pub procedures: Vec<Procedure>,
    pub native_functions: Vec<NativeFunction>,
    pub native_procedures: Vec<NativeProcedure>,
    pub actions: Vec<Action>,
    pub init: Option<Action>,
    pub fsm: Option<ScheduleFsm>,
    pub priorities: Vec<ActionPriority>,
    pub fsm_enums: Vec<FsmEnumeration>,
}

#[derive(Debug, Clone)]
pub struct Unit {
    pub name: String,
    pub functions: Vec<Function>,
    pub procedures: Vec<Procedure>,
    pub vars: Vec<VarDef>,
    pub native_functions: Vec<NativeFunction>,
    pub native_procedures: Vec<NativeProcedure>,
}

#[derive(Debug, Clone)]
pub struct ActorParameter {
    pub name: String,
    pub typ: Type,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Port {
    pub name: String,
    pub typ: Type,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub typ: Type,
}

#[derive(Debug, Clone)]
pub struct Type {
    pub name: String,
    pub non_standard: bool,
    pub size: Option<Box<Expr>>,
    pub list: Option<Box<Type>>,
}

#[derive(Debug, Clone)]
pub struct VarDef {
    pub name: String,
    pub typ: Type,
    pub arrays: Vec<Expr>,
    pub const_assign: bool,
    pub assign: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Generator {
    pub identifier: String,
    pub typ: Option<Type>,
    pub start: Expr,
    pub end: Expr,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub ret_type: Type,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct NativeFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub ret_type: Type,
}

#[derive(Debug, Clone)]
pub struct NativeProcedure {
    pub name: String,
    pub parameters: Vec<Parameter>,
}

#[derive(Debug, Clone)]
pub struct Procedure {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub vars: Vec<VarDef>,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub guards: Vec<Expr>,
    pub input_patterns: Vec<InputPattern>,
    pub output_expressions: Vec<OutputExpr>,
    pub vars: Vec<VarDef>,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct InputPattern {
    pub port: String,
    pub ids: Vec<String>,
    pub repeat: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct OutputExpr {
    pub port: String,
    pub expressions: Vec<Expr>,
    pub repeat: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct ScheduleFsm {
    pub initial_state: String,
    pub transitions: Vec<FsmState>,
}

#[derive(Debug, Clone)]
pub struct FsmState {
    pub state: String,
    pub actions: Vec<String>,
    pub next: String,
}

#[derive(Debug, Clone)]
pub struct FsmEnumeration {
    pub name: String,
    pub initial_state: String,
    pub states: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ActionPriority {
    pub order: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    If {
        cond: Expr,
        then: Vec<Stmt>,
        els: Vec<Stmt>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Block {
        vars: Vec<VarDef>,
        stmts: Vec<Stmt>,
    },
    While {
        cond: Expr,
        vars: Vec<VarDef>,
        stmts: Vec<Stmt>,
    },
    Foreach {
        generators: Vec<Generator>,
        vars: Vec<VarDef>,
        stmts: Vec<Stmt>,
    },
    OutputWrite {
        port: String,
        expr: Expr,
    },
    InputRead {
        port: String,
        identifier: String,
        index: Option<Expr>,
    },
    Assign {
        const_assign: bool,
        identifier: String,
        indices: Vec<Expr>,
        value: Option<Expr>,
    },
    Return,
    TerminateLoop,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Paren(Box<Expr>),
    BinOp {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Literal {
        negation: bool,
        value: String,
    },
    Identifier {
        name: String,
        unary_left: Option<String>,
        unary_right: Option<String>,
        indices: Vec<Expr>,
        call: Option<Vec<Expr>>,
    },
    FsmEnumElement {
        enum_name: String,
        element: String,
    },
    PortPreview {
        port: String,
        prev_identifier: String,
        index: Option<Box<Expr>>,
    },
    PortSize {
        port: String,
    },
    PortFree {
        port: String,
    },
    Ternary {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    ListComprehension {
        expressions: Vec<Expr>,
        generators: Vec<Generator>,
    },
}
