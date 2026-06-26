use crate::ast::{
    Action, ActionPriority, Actor, ActorParameter, Ast, Expr, FsmEnumeration, FsmState, Function,
    Generator, Import, InputPattern, Item, NativeFunction, NativeProcedure, OutputExpr, Parameter,
    Port, Procedure, ScheduleFsm, Stmt, Type, Unit, VarDef,
};
use crate::ffi::ffi;

fn collect<T>(len: usize, f: impl FnMut(usize) -> T) -> Vec<T> {
    (0..len).map(f).collect()
}

fn opt_str(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

pub fn convert(ast: &ffi::Ast) -> Ast {
    let imports = collect(ffi::import_len(ast), |i| {
        convert_import(ffi::import_at(ast, i))
    });

    let item = if ffi::has_actor(ast) {
        Item::Actor(Box::new(convert_actor(ffi::actor(ast))))
    } else {
        Item::Unit(convert_unit(ffi::unit(ast)))
    };

    Ast { imports, item }
}

fn convert_import(x: &ffi::Import) -> Import {
    Import {
        path: ffi::import_path(x),
        symbol: ffi::import_symbol(x),
    }
}

fn convert_actor(x: &ffi::Actor) -> Actor {
    Actor {
        name: ffi::actor_name(x),
        parameters: collect(ffi::actor_param_len(x), |i| {
            convert_actor_parameter(ffi::actor_param_at(x, i))
        }),
        inports: collect(ffi::actor_inport_len(x), |i| {
            convert_port(ffi::actor_inport_at(x, i))
        }),
        outports: collect(ffi::actor_outport_len(x), |i| {
            convert_port(ffi::actor_outport_at(x, i))
        }),
        vars: collect(ffi::actor_var_len(x), |i| {
            convert_vardef(ffi::actor_var_at(x, i))
        }),
        functions: collect(ffi::actor_function_len(x), |i| {
            convert_function(ffi::actor_function_at(x, i))
        }),
        procedures: collect(ffi::actor_procedure_len(x), |i| {
            convert_procedure(ffi::actor_procedure_at(x, i))
        }),
        native_functions: collect(ffi::actor_native_function_len(x), |i| {
            convert_native_function(ffi::actor_native_function_at(x, i))
        }),
        native_procedures: collect(ffi::actor_native_procedure_len(x), |i| {
            convert_native_procedure(ffi::actor_native_procedure_at(x, i))
        }),
        actions: collect(ffi::actor_action_len(x), |i| {
            convert_action(ffi::actor_action_at(x, i))
        }),
        init: ffi::actor_has_init(x).then(|| convert_action(ffi::actor_init(x))),
        fsm: ffi::actor_has_fsm(x).then(|| convert_fsm(ffi::actor_fsm(x))),
        priorities: collect(ffi::actor_priority_len(x), |i| {
            convert_priority(ffi::actor_priority_at(x, i))
        }),
        fsm_enums: collect(ffi::actor_fsm_enum_len(x), |i| {
            convert_fsm_enum(ffi::actor_fsm_enum_at(x, i))
        }),
    }
}

fn convert_unit(x: &ffi::Unit) -> Unit {
    Unit {
        name: ffi::unit_name(x),
        functions: collect(ffi::unit_function_len(x), |i| {
            convert_function(ffi::unit_function_at(x, i))
        }),
        procedures: collect(ffi::unit_procedure_len(x), |i| {
            convert_procedure(ffi::unit_procedure_at(x, i))
        }),
        vars: collect(ffi::unit_var_len(x), |i| {
            convert_vardef(ffi::unit_var_at(x, i))
        }),
        native_functions: collect(ffi::unit_native_function_len(x), |i| {
            convert_native_function(ffi::unit_native_function_at(x, i))
        }),
        native_procedures: collect(ffi::unit_native_procedure_len(x), |i| {
            convert_native_procedure(ffi::unit_native_procedure_at(x, i))
        }),
    }
}

fn convert_actor_parameter(x: &ffi::ActorParameter) -> ActorParameter {
    ActorParameter {
        name: ffi::actorparam_name(x),
        typ: convert_type(ffi::actorparam_type(x)),
        default: ffi::actorparam_has_default(x)
            .then(|| convert_expression(ffi::actorparam_default(x))),
    }
}

fn convert_port(x: &ffi::Port) -> Port {
    Port {
        name: ffi::port_name(x),
        typ: convert_type(ffi::port_type(x)),
    }
}

fn convert_parameter(x: &ffi::Parameter) -> Parameter {
    Parameter {
        name: ffi::param_name(x),
        typ: convert_type(ffi::param_type(x)),
    }
}

fn convert_type(x: &ffi::Type) -> Type {
    Type {
        name: ffi::type_name(x),
        non_standard: ffi::type_non_standard(x),
        size: ffi::type_has_size(x).then(|| Box::new(convert_expression(ffi::type_size(x)))),
        list: ffi::type_has_list(x).then(|| Box::new(convert_type(ffi::type_list(x)))),
    }
}

fn convert_vardef(x: &ffi::VarDefinition) -> VarDef {
    VarDef {
        name: ffi::vardef_name(x),
        typ: convert_type(ffi::vardef_type(x)),
        arrays: collect(ffi::vardef_array_len(x), |i| {
            convert_expression(ffi::vardef_array_at(x, i))
        }),
        const_assign: ffi::vardef_const_assign(x),
        assign: ffi::vardef_has_assign(x).then(|| convert_expression(ffi::vardef_assign(x))),
    }
}

fn convert_generator(x: &ffi::Generator) -> Generator {
    Generator {
        identifier: ffi::generator_identifier(x),
        typ: ffi::generator_has_type(x).then(|| convert_type(ffi::generator_type(x))),
        start: convert_expression(ffi::generator_start(x)),
        end: convert_expression(ffi::generator_end(x)),
    }
}

fn convert_function(x: &ffi::Function) -> Function {
    Function {
        name: ffi::function_name(x),
        parameters: collect(ffi::function_param_len(x), |i| {
            convert_parameter(ffi::function_param_at(x, i))
        }),
        ret_type: convert_type(ffi::function_ret_type(x)),
        expr: convert_expression(ffi::function_expr(x)),
    }
}

fn convert_native_function(x: &ffi::NativeFunction) -> NativeFunction {
    NativeFunction {
        name: ffi::native_function_name(x),
        parameters: collect(ffi::native_function_param_len(x), |i| {
            convert_parameter(ffi::native_function_param_at(x, i))
        }),
        ret_type: convert_type(ffi::native_function_ret_type(x)),
    }
}

fn convert_native_procedure(x: &ffi::NativeProcedure) -> NativeProcedure {
    NativeProcedure {
        name: ffi::native_procedure_name(x),
        parameters: collect(ffi::native_procedure_param_len(x), |i| {
            convert_parameter(ffi::native_procedure_param_at(x, i))
        }),
    }
}

fn convert_procedure(x: &ffi::Procedure) -> Procedure {
    Procedure {
        name: ffi::procedure_name(x),
        parameters: collect(ffi::procedure_param_len(x), |i| {
            convert_parameter(ffi::procedure_param_at(x, i))
        }),
        vars: collect(ffi::procedure_var_len(x), |i| {
            convert_vardef(ffi::procedure_var_at(x, i))
        }),
        stmts: collect(ffi::procedure_stmt_len(x), |i| {
            convert_stmt(ffi::procedure_stmt_at(x, i))
        }),
    }
}

fn convert_action(x: &ffi::Action) -> Action {
    Action {
        name: ffi::action_name(x),
        guards: collect(ffi::action_guard_len(x), |i| {
            convert_expression(ffi::action_guard_at(x, i))
        }),
        input_patterns: collect(ffi::action_input_pattern_len(x), |i| {
            convert_input_pattern(ffi::action_input_pattern_at(x, i))
        }),
        output_expressions: collect(ffi::action_output_expr_len(x), |i| {
            convert_output_expr(ffi::action_output_expr_at(x, i))
        }),
        vars: collect(ffi::action_var_len(x), |i| {
            convert_vardef(ffi::action_var_at(x, i))
        }),
        stmts: collect(ffi::action_stmt_len(x), |i| {
            convert_stmt(ffi::action_stmt_at(x, i))
        }),
    }
}

fn convert_input_pattern(x: &ffi::Input_Pattern) -> InputPattern {
    InputPattern {
        port: ffi::input_pattern_port(x),
        ids: collect(ffi::input_pattern_id_len(x), |i| {
            ffi::input_pattern_id_at(x, i)
        }),
        repeat: ffi::input_pattern_has_repeat(x)
            .then(|| convert_expression(ffi::input_pattern_repeat(x))),
    }
}

fn convert_output_expr(x: &ffi::Output_Expression) -> OutputExpr {
    OutputExpr {
        port: ffi::output_expr_port(x),
        expressions: collect(ffi::output_expr_expr_len(x), |i| {
            convert_expression(ffi::output_expr_expr_at(x, i))
        }),
        repeat: ffi::output_expr_has_repeat(x)
            .then(|| convert_expression(ffi::output_expr_repeat(x))),
    }
}

fn convert_fsm(x: &ffi::Schedule_FSM) -> ScheduleFsm {
    ScheduleFsm {
        initial_state: ffi::fsm_initial_state(x),
        transitions: collect(ffi::fsm_transition_len(x), |i| {
            convert_fsm_state(ffi::fsm_transition_at(x, i))
        }),
    }
}

fn convert_fsm_state(x: &ffi::Schedule_FSM_State) -> FsmState {
    FsmState {
        state: ffi::fsm_state_name(x),
        actions: collect(ffi::fsm_state_action_len(x), |i| {
            ffi::fsm_state_action_at(x, i)
        }),
        next: ffi::fsm_state_next(x),
    }
}

fn convert_fsm_enum(x: &ffi::FSM_Enumeration) -> FsmEnumeration {
    FsmEnumeration {
        name: ffi::fsm_enum_name(x),
        initial_state: ffi::fsm_enum_initial_state(x),
        states: collect(ffi::fsm_enum_state_len(x), |i| ffi::fsm_enum_state_at(x, i)),
    }
}

fn convert_priority(x: &ffi::Action_Priority) -> ActionPriority {
    ActionPriority {
        order: collect(ffi::priority_len(x), |i| ffi::priority_at(x, i)),
    }
}

fn convert_stmt(s: &ffi::Statement) -> Stmt {
    match ffi::stmt_kind(s) {
        0 => {
            let x = ffi::as_if(s);
            Stmt::If {
                cond: convert_expression(ffi::if_cond(x)),
                then: collect(ffi::if_then_len(x), |i| convert_stmt(ffi::if_then_at(x, i))),
                els: if ffi::if_has_else(x) {
                    collect(ffi::if_else_len(x), |i| convert_stmt(ffi::if_else_at(x, i)))
                } else {
                    Vec::new()
                },
            }
        }
        1 => {
            let x = ffi::as_call_stmt(s);
            Stmt::Call {
                name: ffi::call_stmt_name(x),
                args: collect(ffi::call_stmt_arg_len(x), |i| {
                    convert_expression(ffi::call_stmt_arg_at(x, i))
                }),
            }
        }
        2 => {
            let x = ffi::as_block(s);
            Stmt::Block {
                vars: collect(ffi::block_var_len(x), |i| {
                    convert_vardef(ffi::block_var_at(x, i))
                }),
                stmts: collect(ffi::block_stmt_len(x), |i| {
                    convert_stmt(ffi::block_stmt_at(x, i))
                }),
            }
        }
        3 => {
            let x = ffi::as_while(s);
            Stmt::While {
                cond: convert_expression(ffi::while_cond(x)),
                vars: collect(ffi::while_var_len(x), |i| {
                    convert_vardef(ffi::while_var_at(x, i))
                }),
                stmts: collect(ffi::while_stmt_len(x), |i| {
                    convert_stmt(ffi::while_stmt_at(x, i))
                }),
            }
        }
        4 => {
            let x = ffi::as_foreach(s);
            Stmt::Foreach {
                generators: collect(ffi::foreach_generator_len(x), |i| {
                    convert_generator(ffi::foreach_generator_at(x, i))
                }),
                vars: collect(ffi::foreach_var_len(x), |i| {
                    convert_vardef(ffi::foreach_var_at(x, i))
                }),
                stmts: collect(ffi::foreach_stmt_len(x), |i| {
                    convert_stmt(ffi::foreach_stmt_at(x, i))
                }),
            }
        }
        5 => {
            let x = ffi::as_output_write(s);
            Stmt::OutputWrite {
                port: ffi::output_write_port(x),
                expr: convert_expression(ffi::output_write_expr(x)),
            }
        }
        6 => {
            let x = ffi::as_input_read(s);
            Stmt::InputRead {
                port: ffi::input_read_port(x),
                identifier: ffi::input_read_identifier(x),
                index: ffi::input_read_has_index(x)
                    .then(|| convert_index(ffi::input_read_index(x))),
            }
        }
        7 => {
            let x = ffi::as_assignment(s);
            Stmt::Assign {
                const_assign: ffi::assign_const(x),
                identifier: ffi::assign_identifier(x),
                indices: collect(ffi::assign_index_len(x), |i| {
                    convert_index(ffi::assign_index_at(x, i))
                }),
                value: ffi::assign_has_value(x).then(|| convert_expression(ffi::assign_value(x))),
            }
        }
        8 => Stmt::Return,
        _ => Stmt::TerminateLoop,
    }
}

fn convert_index(x: &ffi::Index) -> Expr {
    convert_expression(ffi::index_expr(x))
}

fn convert_expression(x: &ffi::Expression) -> Expr {
    let inner = convert_base_expression(ffi::expression_child(x));
    if ffi::expression_brackets(x) {
        Expr::Paren(Box::new(inner))
    } else {
        inner
    }
}

fn convert_base_expression(e: &ffi::BaseExpression) -> Expr {
    match ffi::expr_kind(e) {
        0 => convert_expression(ffi::as_expression(e)),
        1 => {
            let x = ffi::as_operator(e);
            Expr::BinOp {
                op: ffi::operator_ops(x),
                left: Box::new(convert_base_expression(ffi::operator_left(x))),
                right: Box::new(convert_base_expression(ffi::operator_right(x))),
            }
        }
        2 => {
            let x = ffi::as_literal(e);
            Expr::Literal {
                negation: ffi::literal_negation(x),
                value: ffi::literal_value(x),
            }
        }
        3 => {
            let x = ffi::as_identifier(e);
            Expr::Identifier {
                name: ffi::identifier_name(x),
                unary_left: opt_str(ffi::identifier_unary_left(x)),
                unary_right: opt_str(ffi::identifier_unary_right(x)),
                indices: collect(ffi::identifier_index_len(x), |i| {
                    convert_index(ffi::identifier_index_at(x, i))
                }),
                call: ffi::identifier_has_call(x).then(|| {
                    collect(ffi::identifier_call_len(x), |i| {
                        convert_expression(ffi::identifier_call_at(x, i))
                    })
                }),
            }
        }
        4 => {
            let x = ffi::as_fsm_enum_element(e);
            Expr::FsmEnumElement {
                enum_name: ffi::fsm_elem_enum_name(x),
                element: ffi::fsm_elem_element(x),
            }
        }
        5 => {
            let x = ffi::as_port_preview(e);
            Expr::PortPreview {
                port: ffi::port_preview_port(x),
                prev_identifier: ffi::port_preview_prev_identifier(x),
                index: ffi::port_preview_has_index(x)
                    .then(|| Box::new(convert_index(ffi::port_preview_index(x)))),
            }
        }
        6 => Expr::PortSize {
            port: ffi::port_size_port(ffi::as_port_size(e)),
        },
        7 => Expr::PortFree {
            port: ffi::port_free_port(ffi::as_port_free(e)),
        },
        8 => {
            let x = ffi::as_ternary(e);
            Expr::Ternary {
                cond: Box::new(convert_expression(ffi::ternary_cond(x))),
                then: Box::new(convert_expression(ffi::ternary_then(x))),
                els: Box::new(convert_expression(ffi::ternary_else(x))),
            }
        }
        _ => {
            let x = ffi::as_list_comprehension(e);
            Expr::ListComprehension {
                expressions: collect(ffi::listcomp_expr_len(x), |i| {
                    convert_expression(ffi::listcomp_expr_at(x, i))
                }),
                generators: collect(ffi::listcomp_generator_len(x), |i| {
                    convert_generator(ffi::listcomp_generator_at(x, i))
                }),
            }
        }
    }
}
