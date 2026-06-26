#![allow(unused)]

#[allow(clippy::module_inception)]
#[cxx::bridge(namespace = "shim")]
pub mod ffi {
    #[namespace = "AST"]
    unsafe extern "C++" {
        type Import;
        type Actor;
        type Unit;
        type ActorParameter;
        type Port;
        type Parameter;
        type Type;
        type VarDefinition;
        type Generator;
        type Index;
        type Function;
        type NativeFunction;
        type NativeProcedure;
        type Procedure;
        type Action;
        type Input_Pattern;
        type Output_Expression;
        type Schedule_FSM;
        type Schedule_FSM_State;
        type FSM_Enumeration;
        type Action_Priority;
        type Statement;
        type IfStatement;
        type CallStatement;
        type BlockStatement;
        type WhileStatement;
        type ForeachStatement;
        type OutputChannelWriteStatement;
        type InputChannelReadStatement;
        type AssignmentStatement;
        type BaseExpression;
        type Expression;
        type Operator;
        type Literal;
        type Identifier;
        type FSM_Enumeration_Element;
        type PortPreview;
        type PortSize;
        type PortFree;
        type TernaryOperator;
        type ListComprehension;
    }

    unsafe extern "C++" {
        include!("cpp/shim.hpp");

        type Ast;

        fn parse_cal(code: &str) -> Result<UniquePtr<Ast>>;

        fn import_len(a: &Ast) -> usize;
        fn import_at(a: &Ast, i: usize) -> &Import;
        fn has_actor(a: &Ast) -> bool;
        fn actor(a: &Ast) -> &Actor;
        fn has_unit(a: &Ast) -> bool;
        fn unit(a: &Ast) -> &Unit;

        fn import_path(x: &Import) -> String;
        fn import_symbol(x: &Import) -> String;

        fn actor_name(x: &Actor) -> String;
        fn actor_param_len(x: &Actor) -> usize;
        fn actor_param_at(x: &Actor, i: usize) -> &ActorParameter;
        fn actor_inport_len(x: &Actor) -> usize;
        fn actor_inport_at(x: &Actor, i: usize) -> &Port;
        fn actor_outport_len(x: &Actor) -> usize;
        fn actor_outport_at(x: &Actor, i: usize) -> &Port;
        fn actor_var_len(x: &Actor) -> usize;
        fn actor_var_at(x: &Actor, i: usize) -> &VarDefinition;
        fn actor_function_len(x: &Actor) -> usize;
        fn actor_function_at(x: &Actor, i: usize) -> &Function;
        fn actor_procedure_len(x: &Actor) -> usize;
        fn actor_procedure_at(x: &Actor, i: usize) -> &Procedure;
        fn actor_native_function_len(x: &Actor) -> usize;
        fn actor_native_function_at(x: &Actor, i: usize) -> &NativeFunction;
        fn actor_native_procedure_len(x: &Actor) -> usize;
        fn actor_native_procedure_at(x: &Actor, i: usize) -> &NativeProcedure;
        fn actor_action_len(x: &Actor) -> usize;
        fn actor_action_at(x: &Actor, i: usize) -> &Action;
        fn actor_has_init(x: &Actor) -> bool;
        fn actor_init(x: &Actor) -> &Action;
        fn actor_has_fsm(x: &Actor) -> bool;
        fn actor_fsm(x: &Actor) -> &Schedule_FSM;
        fn actor_priority_len(x: &Actor) -> usize;
        fn actor_priority_at(x: &Actor, i: usize) -> &Action_Priority;
        fn actor_fsm_enum_len(x: &Actor) -> usize;
        fn actor_fsm_enum_at(x: &Actor, i: usize) -> &FSM_Enumeration;

        fn unit_name(x: &Unit) -> String;
        fn unit_function_len(x: &Unit) -> usize;
        fn unit_function_at(x: &Unit, i: usize) -> &Function;
        fn unit_procedure_len(x: &Unit) -> usize;
        fn unit_procedure_at(x: &Unit, i: usize) -> &Procedure;
        fn unit_var_len(x: &Unit) -> usize;
        fn unit_var_at(x: &Unit, i: usize) -> &VarDefinition;
        fn unit_native_function_len(x: &Unit) -> usize;
        fn unit_native_function_at(x: &Unit, i: usize) -> &NativeFunction;
        fn unit_native_procedure_len(x: &Unit) -> usize;
        fn unit_native_procedure_at(x: &Unit, i: usize) -> &NativeProcedure;

        fn actorparam_name(x: &ActorParameter) -> String;
        fn actorparam_type(x: &ActorParameter) -> &Type;
        fn actorparam_has_default(x: &ActorParameter) -> bool;
        fn actorparam_default(x: &ActorParameter) -> &Expression;

        fn port_name(x: &Port) -> String;
        fn port_type(x: &Port) -> &Type;

        fn param_name(x: &Parameter) -> String;
        fn param_type(x: &Parameter) -> &Type;

        fn type_name(x: &Type) -> String;
        fn type_non_standard(x: &Type) -> bool;
        fn type_has_size(x: &Type) -> bool;
        fn type_size(x: &Type) -> &Expression;
        fn type_has_list(x: &Type) -> bool;
        fn type_list(x: &Type) -> &Type;

        fn vardef_name(x: &VarDefinition) -> String;
        fn vardef_type(x: &VarDefinition) -> &Type;
        fn vardef_const_assign(x: &VarDefinition) -> bool;
        fn vardef_array_len(x: &VarDefinition) -> usize;
        fn vardef_array_at(x: &VarDefinition, i: usize) -> &Expression;
        fn vardef_has_assign(x: &VarDefinition) -> bool;
        fn vardef_assign(x: &VarDefinition) -> &Expression;

        fn generator_identifier(x: &Generator) -> String;
        fn generator_has_type(x: &Generator) -> bool;
        fn generator_type(x: &Generator) -> &Type;
        fn generator_start(x: &Generator) -> &Expression;
        fn generator_end(x: &Generator) -> &Expression;

        fn index_has_expr(x: &Index) -> bool;
        fn index_expr(x: &Index) -> &Expression;

        fn function_name(x: &Function) -> String;
        fn function_param_len(x: &Function) -> usize;
        fn function_param_at(x: &Function, i: usize) -> &Parameter;
        fn function_ret_type(x: &Function) -> &Type;
        fn function_expr(x: &Function) -> &Expression;
        fn native_function_name(x: &NativeFunction) -> String;
        fn native_function_param_len(x: &NativeFunction) -> usize;
        fn native_function_param_at(x: &NativeFunction, i: usize) -> &Parameter;
        fn native_function_ret_type(x: &NativeFunction) -> &Type;
        fn native_procedure_name(x: &NativeProcedure) -> String;
        fn native_procedure_param_len(x: &NativeProcedure) -> usize;
        fn native_procedure_param_at(x: &NativeProcedure, i: usize) -> &Parameter;
        fn procedure_name(x: &Procedure) -> String;
        fn procedure_param_len(x: &Procedure) -> usize;
        fn procedure_param_at(x: &Procedure, i: usize) -> &Parameter;
        fn procedure_var_len(x: &Procedure) -> usize;
        fn procedure_var_at(x: &Procedure, i: usize) -> &VarDefinition;
        fn procedure_stmt_len(x: &Procedure) -> usize;
        fn procedure_stmt_at(x: &Procedure, i: usize) -> &Statement;

        fn action_name(x: &Action) -> String;
        fn action_guard_len(x: &Action) -> usize;
        fn action_guard_at(x: &Action, i: usize) -> &Expression;
        fn action_input_pattern_len(x: &Action) -> usize;
        fn action_input_pattern_at(x: &Action, i: usize) -> &Input_Pattern;
        fn action_output_expr_len(x: &Action) -> usize;
        fn action_output_expr_at(x: &Action, i: usize) -> &Output_Expression;
        fn action_var_len(x: &Action) -> usize;
        fn action_var_at(x: &Action, i: usize) -> &VarDefinition;
        fn action_stmt_len(x: &Action) -> usize;
        fn action_stmt_at(x: &Action, i: usize) -> &Statement;

        fn input_pattern_port(x: &Input_Pattern) -> String;
        fn input_pattern_id_len(x: &Input_Pattern) -> usize;
        fn input_pattern_id_at(x: &Input_Pattern, i: usize) -> String;
        fn input_pattern_has_repeat(x: &Input_Pattern) -> bool;
        fn input_pattern_repeat(x: &Input_Pattern) -> &Expression;
        fn output_expr_port(x: &Output_Expression) -> String;
        fn output_expr_expr_len(x: &Output_Expression) -> usize;
        fn output_expr_expr_at(x: &Output_Expression, i: usize) -> &Expression;
        fn output_expr_has_repeat(x: &Output_Expression) -> bool;
        fn output_expr_repeat(x: &Output_Expression) -> &Expression;

        fn fsm_initial_state(x: &Schedule_FSM) -> String;
        fn fsm_transition_len(x: &Schedule_FSM) -> usize;
        fn fsm_transition_at(x: &Schedule_FSM, i: usize) -> &Schedule_FSM_State;
        fn fsm_state_name(x: &Schedule_FSM_State) -> String;
        fn fsm_state_next(x: &Schedule_FSM_State) -> String;
        fn fsm_state_action_len(x: &Schedule_FSM_State) -> usize;
        fn fsm_state_action_at(x: &Schedule_FSM_State, i: usize) -> String;
        fn fsm_enum_name(x: &FSM_Enumeration) -> String;
        fn fsm_enum_initial_state(x: &FSM_Enumeration) -> String;
        fn fsm_enum_state_len(x: &FSM_Enumeration) -> usize;
        fn fsm_enum_state_at(x: &FSM_Enumeration, i: usize) -> String;
        fn priority_len(x: &Action_Priority) -> usize;
        fn priority_at(x: &Action_Priority, i: usize) -> String;

        fn stmt_kind(s: &Statement) -> u8;
        fn as_if(s: &Statement) -> &IfStatement;
        fn as_call_stmt(s: &Statement) -> &CallStatement;
        fn as_block(s: &Statement) -> &BlockStatement;
        fn as_while(s: &Statement) -> &WhileStatement;
        fn as_foreach(s: &Statement) -> &ForeachStatement;
        fn as_output_write(s: &Statement) -> &OutputChannelWriteStatement;
        fn as_input_read(s: &Statement) -> &InputChannelReadStatement;
        fn as_assignment(s: &Statement) -> &AssignmentStatement;

        fn if_cond(x: &IfStatement) -> &Expression;
        fn if_then_len(x: &IfStatement) -> usize;
        fn if_then_at(x: &IfStatement, i: usize) -> &Statement;
        fn if_has_else(x: &IfStatement) -> bool;
        fn if_else_len(x: &IfStatement) -> usize;
        fn if_else_at(x: &IfStatement, i: usize) -> &Statement;

        fn call_stmt_name(x: &CallStatement) -> String;
        fn call_stmt_arg_len(x: &CallStatement) -> usize;
        fn call_stmt_arg_at(x: &CallStatement, i: usize) -> &Expression;

        fn block_var_len(x: &BlockStatement) -> usize;
        fn block_var_at(x: &BlockStatement, i: usize) -> &VarDefinition;
        fn block_stmt_len(x: &BlockStatement) -> usize;
        fn block_stmt_at(x: &BlockStatement, i: usize) -> &Statement;

        fn while_cond(x: &WhileStatement) -> &Expression;
        fn while_var_len(x: &WhileStatement) -> usize;
        fn while_var_at(x: &WhileStatement, i: usize) -> &VarDefinition;
        fn while_stmt_len(x: &WhileStatement) -> usize;
        fn while_stmt_at(x: &WhileStatement, i: usize) -> &Statement;

        fn foreach_generator_len(x: &ForeachStatement) -> usize;
        fn foreach_generator_at(x: &ForeachStatement, i: usize) -> &Generator;
        fn foreach_var_len(x: &ForeachStatement) -> usize;
        fn foreach_var_at(x: &ForeachStatement, i: usize) -> &VarDefinition;
        fn foreach_stmt_len(x: &ForeachStatement) -> usize;
        fn foreach_stmt_at(x: &ForeachStatement, i: usize) -> &Statement;

        fn output_write_port(x: &OutputChannelWriteStatement) -> String;
        fn output_write_expr(x: &OutputChannelWriteStatement) -> &Expression;

        fn input_read_port(x: &InputChannelReadStatement) -> String;
        fn input_read_identifier(x: &InputChannelReadStatement) -> String;
        fn input_read_has_index(x: &InputChannelReadStatement) -> bool;
        fn input_read_index(x: &InputChannelReadStatement) -> &Index;

        fn assign_const(x: &AssignmentStatement) -> bool;
        fn assign_identifier(x: &AssignmentStatement) -> String;
        fn assign_index_len(x: &AssignmentStatement) -> usize;
        fn assign_index_at(x: &AssignmentStatement, i: usize) -> &Index;
        fn assign_has_value(x: &AssignmentStatement) -> bool;
        fn assign_value(x: &AssignmentStatement) -> &Expression;

        fn expr_kind(e: &BaseExpression) -> u8;
        fn as_expression(e: &BaseExpression) -> &Expression;
        fn as_operator(e: &BaseExpression) -> &Operator;
        fn as_literal(e: &BaseExpression) -> &Literal;
        fn as_identifier(e: &BaseExpression) -> &Identifier;
        fn as_fsm_enum_element(e: &BaseExpression) -> &FSM_Enumeration_Element;
        fn as_port_preview(e: &BaseExpression) -> &PortPreview;
        fn as_port_size(e: &BaseExpression) -> &PortSize;
        fn as_port_free(e: &BaseExpression) -> &PortFree;
        fn as_ternary(e: &BaseExpression) -> &TernaryOperator;
        fn as_list_comprehension(e: &BaseExpression) -> &ListComprehension;

        fn expression_brackets(x: &Expression) -> bool;
        fn expression_child(x: &Expression) -> &BaseExpression;

        fn operator_ops(x: &Operator) -> String;
        fn operator_left(x: &Operator) -> &BaseExpression;
        fn operator_right(x: &Operator) -> &BaseExpression;

        fn literal_negation(x: &Literal) -> bool;
        fn literal_value(x: &Literal) -> String;

        fn identifier_name(x: &Identifier) -> String;
        fn identifier_unary_left(x: &Identifier) -> String;
        fn identifier_unary_right(x: &Identifier) -> String;
        fn identifier_index_len(x: &Identifier) -> usize;
        fn identifier_index_at(x: &Identifier, i: usize) -> &Index;
        fn identifier_has_call(x: &Identifier) -> bool;
        fn identifier_call_len(x: &Identifier) -> usize;
        fn identifier_call_at(x: &Identifier, i: usize) -> &Expression;

        fn fsm_elem_enum_name(x: &FSM_Enumeration_Element) -> String;
        fn fsm_elem_element(x: &FSM_Enumeration_Element) -> String;

        fn port_preview_port(x: &PortPreview) -> String;
        fn port_preview_prev_identifier(x: &PortPreview) -> String;
        fn port_preview_has_index(x: &PortPreview) -> bool;
        fn port_preview_index(x: &PortPreview) -> &Index;

        fn port_size_port(x: &PortSize) -> String;
        fn port_free_port(x: &PortFree) -> String;

        fn ternary_cond(x: &TernaryOperator) -> &Expression;
        fn ternary_then(x: &TernaryOperator) -> &Expression;
        fn ternary_else(x: &TernaryOperator) -> &Expression;

        fn listcomp_expr_len(x: &ListComprehension) -> usize;
        fn listcomp_expr_at(x: &ListComprehension, i: usize) -> &Expression;
        fn listcomp_generator_len(x: &ListComprehension) -> usize;
        fn listcomp_generator_at(x: &ListComprehension, i: usize) -> &Generator;
    }
}
