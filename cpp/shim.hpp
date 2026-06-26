#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>

#include "rust/cxx.h"

#include "IR/AST/AST.hpp"

namespace shim
{

    class Ast
    {
    public:
        AST::AST_Root *root;

        explicit Ast(AST::AST_Root *r) : root(r) {}
        ~Ast();

        Ast(const Ast &) = delete;
        Ast &operator=(const Ast &) = delete;
    };

    std::unique_ptr<Ast> parse_cal(rust::Str code);

#define SHIM_STR(fn, T, expr) \
    inline rust::String fn(const AST::T &x) { return rust::String(expr); }
#define SHIM_BOOL(fn, T, expr) \
    inline bool fn(const AST::T &x) { return (expr); }
#define SHIM_HAS(fn, T, member) \
    inline bool fn(const AST::T &x) { return x.member != nullptr; }
#define SHIM_PTR(fn, RT, T, member) \
    inline const AST::RT &fn(const AST::T &x) { return *x.member; }
#define SHIM_OBJ(fn, RT, T, member) \
    inline const AST::RT &fn(const AST::T &x) { return x.member; }
#define SHIM_VEC(fn_len, fn_at, RT, T, member)                             \
    inline std::size_t fn_len(const AST::T &x) { return x.member.size(); } \
    inline const AST::RT &fn_at(const AST::T &x, std::size_t i) { return *x.member[i]; }
#define SHIM_VEC_ID(fn_len, fn_at, T, member)                              \
    inline std::size_t fn_len(const AST::T &x) { return x.member.size(); } \
    inline rust::String fn_at(const AST::T &x, std::size_t i) { return rust::String(x.member[i].name); }
#define SHIM_VEC_STR(fn_len, fn_at, T, member)                             \
    inline std::size_t fn_len(const AST::T &x) { return x.member.size(); } \
    inline rust::String fn_at(const AST::T &x, std::size_t i) { return rust::String(x.member[i]); }

    inline std::size_t import_len(const Ast &a) { return a.root->imports.size(); }
    inline const AST::Import &import_at(const Ast &a, std::size_t i) { return *a.root->imports[i]; }
    inline bool has_actor(const Ast &a) { return a.root->actor != nullptr; }
    inline const AST::Actor &actor(const Ast &a) { return *a.root->actor; }
    inline bool has_unit(const Ast &a) { return a.root->unit != nullptr; }
    inline const AST::Unit &unit(const Ast &a) { return *a.root->unit; }

    SHIM_STR(import_path, Import, x.path.name)
    SHIM_STR(import_symbol, Import, x.symbol.name)

    SHIM_STR(actor_name, Actor, x.name.name)
    SHIM_VEC(actor_param_len, actor_param_at, ActorParameter, Actor, parameters)
    SHIM_VEC(actor_inport_len, actor_inport_at, Port, Actor, inports)
    SHIM_VEC(actor_outport_len, actor_outport_at, Port, Actor, outports)
    SHIM_VEC(actor_var_len, actor_var_at, VarDefinition, Actor, vars)
    SHIM_VEC(actor_function_len, actor_function_at, Function, Actor, functions)
    SHIM_VEC(actor_procedure_len, actor_procedure_at, Procedure, Actor, procedures)
    SHIM_VEC(actor_native_function_len, actor_native_function_at, NativeFunction, Actor, nativefunctions)
    SHIM_VEC(actor_native_procedure_len, actor_native_procedure_at, NativeProcedure, Actor, nativeprocedures)
    SHIM_VEC(actor_action_len, actor_action_at, Action, Actor, actions)
    SHIM_HAS(actor_has_init, Actor, init)
    SHIM_PTR(actor_init, Action, Actor, init)
    SHIM_HAS(actor_has_fsm, Actor, fsm)
    SHIM_PTR(actor_fsm, Schedule_FSM, Actor, fsm)
    SHIM_VEC(actor_priority_len, actor_priority_at, Action_Priority, Actor, prios)
    SHIM_VEC(actor_fsm_enum_len, actor_fsm_enum_at, FSM_Enumeration, Actor, fsm_enums)

    SHIM_STR(unit_name, Unit, x.name.name)
    SHIM_VEC(unit_function_len, unit_function_at, Function, Unit, functions)
    SHIM_VEC(unit_procedure_len, unit_procedure_at, Procedure, Unit, procedures)
    SHIM_VEC(unit_var_len, unit_var_at, VarDefinition, Unit, vars)
    SHIM_VEC(unit_native_function_len, unit_native_function_at, NativeFunction, Unit, nativefunctions)
    SHIM_VEC(unit_native_procedure_len, unit_native_procedure_at, NativeProcedure, Unit, nativeprocedures)

    SHIM_STR(actorparam_name, ActorParameter, x.name.name)
    SHIM_OBJ(actorparam_type, Type, ActorParameter, type)
    SHIM_HAS(actorparam_has_default, ActorParameter, asign)
    SHIM_PTR(actorparam_default, Expression, ActorParameter, asign)

    SHIM_STR(port_name, Port, x.name.name)
    SHIM_OBJ(port_type, Type, Port, type)

    SHIM_STR(param_name, Parameter, x.name.name)
    SHIM_OBJ(param_type, Type, Parameter, type)

    SHIM_STR(type_name, Type, x.type.name)
    SHIM_BOOL(type_non_standard, Type, x.non_standard_type)
    SHIM_HAS(type_has_size, Type, size)
    SHIM_PTR(type_size, Expression, Type, size)
    SHIM_HAS(type_has_list, Type, listtype)
    SHIM_PTR(type_list, Type, Type, listtype)

    SHIM_STR(vardef_name, VarDefinition, x.name.name)
    SHIM_OBJ(vardef_type, Type, VarDefinition, type)
    SHIM_BOOL(vardef_const_assign, VarDefinition, x.constassign)
    SHIM_VEC(vardef_array_len, vardef_array_at, Expression, VarDefinition, arrays)
    SHIM_HAS(vardef_has_assign, VarDefinition, assign)
    SHIM_PTR(vardef_assign, Expression, VarDefinition, assign)

    SHIM_STR(generator_identifier, Generator, x.identifier.name)
    SHIM_HAS(generator_has_type, Generator, type)
    SHIM_PTR(generator_type, Type, Generator, type)
    SHIM_PTR(generator_start, Expression, Generator, start)
    SHIM_PTR(generator_end, Expression, Generator, end)

    SHIM_HAS(index_has_expr, Index, index)
    SHIM_PTR(index_expr, Expression, Index, index)

    SHIM_STR(function_name, Function, x.name.name)
    SHIM_VEC(function_param_len, function_param_at, Parameter, Function, parameters)
    SHIM_OBJ(function_ret_type, Type, Function, ret_type)
    SHIM_PTR(function_expr, Expression, Function, expression)

    SHIM_STR(native_function_name, NativeFunction, x.name.name)
    SHIM_VEC(native_function_param_len, native_function_param_at, Parameter, NativeFunction, parameters)
    SHIM_OBJ(native_function_ret_type, Type, NativeFunction, ret_type)

    SHIM_STR(native_procedure_name, NativeProcedure, x.name.name)
    SHIM_VEC(native_procedure_param_len, native_procedure_param_at, Parameter, NativeProcedure, parameters)

    SHIM_STR(procedure_name, Procedure, x.name.name)
    SHIM_VEC(procedure_param_len, procedure_param_at, Parameter, Procedure, parameters)
    SHIM_VEC(procedure_var_len, procedure_var_at, VarDefinition, Procedure, vars)
    SHIM_VEC(procedure_stmt_len, procedure_stmt_at, Statement, Procedure, statements)

    SHIM_STR(action_name, Action, x.name.name)
    SHIM_VEC(action_guard_len, action_guard_at, Expression, Action, guards)
    SHIM_VEC(action_input_pattern_len, action_input_pattern_at, Input_Pattern, Action, input_patterns)
    SHIM_VEC(action_output_expr_len, action_output_expr_at, Output_Expression, Action, output_expressions)
    SHIM_VEC(action_var_len, action_var_at, VarDefinition, Action, vars)
    SHIM_VEC(action_stmt_len, action_stmt_at, Statement, Action, statements)

    SHIM_STR(input_pattern_port, Input_Pattern, x.port.name)
    SHIM_VEC_ID(input_pattern_id_len, input_pattern_id_at, Input_Pattern, IDs)
    SHIM_HAS(input_pattern_has_repeat, Input_Pattern, repeat)
    SHIM_PTR(input_pattern_repeat, Expression, Input_Pattern, repeat)

    SHIM_STR(output_expr_port, Output_Expression, x.port.name)
    SHIM_VEC(output_expr_expr_len, output_expr_expr_at, Expression, Output_Expression, expressions)
    SHIM_HAS(output_expr_has_repeat, Output_Expression, repeat)
    SHIM_PTR(output_expr_repeat, Expression, Output_Expression, repeat)

    SHIM_STR(fsm_initial_state, Schedule_FSM, x.inital_state.name)
    SHIM_VEC(fsm_transition_len, fsm_transition_at, Schedule_FSM_State, Schedule_FSM, transitions)

    SHIM_STR(fsm_state_name, Schedule_FSM_State, x.state.name)
    SHIM_STR(fsm_state_next, Schedule_FSM_State, x.next.name)
    SHIM_VEC_ID(fsm_state_action_len, fsm_state_action_at, Schedule_FSM_State, actions)

    SHIM_STR(fsm_enum_name, FSM_Enumeration, x.name)
    SHIM_STR(fsm_enum_initial_state, FSM_Enumeration, x.inital_state)
    SHIM_VEC_STR(fsm_enum_state_len, fsm_enum_state_at, FSM_Enumeration, states)

    SHIM_VEC_ID(priority_len, priority_at, Action_Priority, prio_rel)

    inline std::uint8_t stmt_kind(const AST::Statement &s)
    {
        if (dynamic_cast<const AST::IfStatement *>(&s))
            return 0;
        if (dynamic_cast<const AST::CallStatement *>(&s))
            return 1;
        if (dynamic_cast<const AST::BlockStatement *>(&s))
            return 2;
        if (dynamic_cast<const AST::WhileStatement *>(&s))
            return 3;
        if (dynamic_cast<const AST::ForeachStatement *>(&s))
            return 4;
        if (dynamic_cast<const AST::OutputChannelWriteStatement *>(&s))
            return 5;
        if (dynamic_cast<const AST::InputChannelReadStatement *>(&s))
            return 6;
        if (dynamic_cast<const AST::AssignmentStatement *>(&s))
            return 7;
        if (dynamic_cast<const AST::ReturnStatement *>(&s))
            return 8;
        return 9;
    }

    inline const AST::IfStatement &as_if(const AST::Statement &s) { return static_cast<const AST::IfStatement &>(s); }
    inline const AST::CallStatement &as_call_stmt(const AST::Statement &s) { return static_cast<const AST::CallStatement &>(s); }
    inline const AST::BlockStatement &as_block(const AST::Statement &s) { return static_cast<const AST::BlockStatement &>(s); }
    inline const AST::WhileStatement &as_while(const AST::Statement &s) { return static_cast<const AST::WhileStatement &>(s); }
    inline const AST::ForeachStatement &as_foreach(const AST::Statement &s) { return static_cast<const AST::ForeachStatement &>(s); }
    inline const AST::OutputChannelWriteStatement &as_output_write(const AST::Statement &s) { return static_cast<const AST::OutputChannelWriteStatement &>(s); }
    inline const AST::InputChannelReadStatement &as_input_read(const AST::Statement &s) { return static_cast<const AST::InputChannelReadStatement &>(s); }
    inline const AST::AssignmentStatement &as_assignment(const AST::Statement &s) { return static_cast<const AST::AssignmentStatement &>(s); }

    SHIM_PTR(if_cond, Expression, IfStatement, condition)
    SHIM_VEC(if_then_len, if_then_at, Statement, IfStatement, ifblock)
    SHIM_HAS(if_has_else, IfStatement, elseblock)
    inline std::size_t if_else_len(const AST::IfStatement &x) { return x.elseblock->statements.size(); }
    inline const AST::Statement &if_else_at(const AST::IfStatement &x, std::size_t i) { return *x.elseblock->statements[i]; }

    SHIM_STR(call_stmt_name, CallStatement, x.name.name)
    SHIM_VEC(call_stmt_arg_len, call_stmt_arg_at, Expression, CallStatement, parameters)

    SHIM_VEC(block_var_len, block_var_at, VarDefinition, BlockStatement, vars)
    SHIM_VEC(block_stmt_len, block_stmt_at, Statement, BlockStatement, statements)

    SHIM_PTR(while_cond, Expression, WhileStatement, condition)
    SHIM_VEC(while_var_len, while_var_at, VarDefinition, WhileStatement, vars)
    SHIM_VEC(while_stmt_len, while_stmt_at, Statement, WhileStatement, statements)

    SHIM_VEC(foreach_generator_len, foreach_generator_at, Generator, ForeachStatement, generators)
    SHIM_VEC(foreach_var_len, foreach_var_at, VarDefinition, ForeachStatement, vars)
    SHIM_VEC(foreach_stmt_len, foreach_stmt_at, Statement, ForeachStatement, statements)

    SHIM_STR(output_write_port, OutputChannelWriteStatement, x.port.name)
    SHIM_PTR(output_write_expr, Expression, OutputChannelWriteStatement, expr)

    SHIM_STR(input_read_port, InputChannelReadStatement, x.port.name)
    SHIM_STR(input_read_identifier, InputChannelReadStatement, x.identifier.name)
    SHIM_HAS(input_read_has_index, InputChannelReadStatement, index)
    SHIM_PTR(input_read_index, Index, InputChannelReadStatement, index)

    SHIM_BOOL(assign_const, AssignmentStatement, x.constasgn)
    SHIM_STR(assign_identifier, AssignmentStatement, x.identifier.name)
    SHIM_VEC(assign_index_len, assign_index_at, Index, AssignmentStatement, indices)
    SHIM_HAS(assign_has_value, AssignmentStatement, asgnvalue)
    SHIM_PTR(assign_value, Expression, AssignmentStatement, asgnvalue)

    inline std::uint8_t expr_kind(const AST::BaseExpression &e)
    {
        if (dynamic_cast<const AST::Expression *>(&e))
            return 0;
        if (dynamic_cast<const AST::Operator *>(&e))
            return 1;
        if (dynamic_cast<const AST::Literal *>(&e))
            return 2;
        if (dynamic_cast<const AST::Identifier *>(&e))
            return 3;
        if (dynamic_cast<const AST::FSM_Enumeration_Element *>(&e))
            return 4;
        if (dynamic_cast<const AST::PortPreview *>(&e))
            return 5;
        if (dynamic_cast<const AST::PortSize *>(&e))
            return 6;
        if (dynamic_cast<const AST::PortFree *>(&e))
            return 7;
        if (dynamic_cast<const AST::TernaryOperator *>(&e))
            return 8;
        return 9;
    }

    inline const AST::Expression &as_expression(const AST::BaseExpression &e) { return static_cast<const AST::Expression &>(e); }
    inline const AST::Operator &as_operator(const AST::BaseExpression &e) { return static_cast<const AST::Operator &>(e); }
    inline const AST::Literal &as_literal(const AST::BaseExpression &e) { return static_cast<const AST::Literal &>(e); }
    inline const AST::Identifier &as_identifier(const AST::BaseExpression &e) { return static_cast<const AST::Identifier &>(e); }
    inline const AST::FSM_Enumeration_Element &as_fsm_enum_element(const AST::BaseExpression &e) { return static_cast<const AST::FSM_Enumeration_Element &>(e); }
    inline const AST::PortPreview &as_port_preview(const AST::BaseExpression &e) { return static_cast<const AST::PortPreview &>(e); }
    inline const AST::PortSize &as_port_size(const AST::BaseExpression &e) { return static_cast<const AST::PortSize &>(e); }
    inline const AST::PortFree &as_port_free(const AST::BaseExpression &e) { return static_cast<const AST::PortFree &>(e); }
    inline const AST::TernaryOperator &as_ternary(const AST::BaseExpression &e) { return static_cast<const AST::TernaryOperator &>(e); }
    inline const AST::ListComprehension &as_list_comprehension(const AST::BaseExpression &e) { return static_cast<const AST::ListComprehension &>(e); }

    SHIM_BOOL(expression_brackets, Expression, x.brakets)
    SHIM_PTR(expression_child, BaseExpression, Expression, child)

    SHIM_STR(operator_ops, Operator, x.ops)
    SHIM_PTR(operator_left, BaseExpression, Operator, left)
    SHIM_PTR(operator_right, BaseExpression, Operator, right)

    SHIM_BOOL(literal_negation, Literal, x.negation)
    SHIM_STR(literal_value, Literal, x.literal)

    SHIM_STR(identifier_name, Identifier, x.identifier)
    inline rust::String identifier_unary_left(const AST::Identifier &x) { return rust::String(x.unary_left ? x.unary_left->ops : std::string()); }
    inline rust::String identifier_unary_right(const AST::Identifier &x) { return rust::String(x.unary_right ? x.unary_right->ops : std::string()); }
    SHIM_VEC(identifier_index_len, identifier_index_at, Index, Identifier, indices)
    SHIM_HAS(identifier_has_call, Identifier, call)
    inline std::size_t identifier_call_len(const AST::Identifier &x) { return x.call->parameters.size(); }
    inline const AST::Expression &identifier_call_at(const AST::Identifier &x, std::size_t i) { return *x.call->parameters[i]; }

    SHIM_STR(fsm_elem_enum_name, FSM_Enumeration_Element, x.enum_name)
    SHIM_STR(fsm_elem_element, FSM_Enumeration_Element, x.enum_element)

    SHIM_STR(port_preview_port, PortPreview, x.port)
    SHIM_STR(port_preview_prev_identifier, PortPreview, x.prev_identifier)
    SHIM_HAS(port_preview_has_index, PortPreview, index)
    SHIM_PTR(port_preview_index, Index, PortPreview, index)

    SHIM_STR(port_size_port, PortSize, x.port)
    SHIM_STR(port_free_port, PortFree, x.port)

    SHIM_PTR(ternary_cond, Expression, TernaryOperator, cond)
    SHIM_PTR(ternary_then, Expression, TernaryOperator, ifblock)
    SHIM_PTR(ternary_else, Expression, TernaryOperator, elseblock)

    SHIM_VEC(listcomp_expr_len, listcomp_expr_at, Expression, ListComprehension, expressions)
    SHIM_VEC(listcomp_generator_len, listcomp_generator_at, Generator, ListComprehension, generators)

#undef SHIM_STR
#undef SHIM_BOOL
#undef SHIM_HAS
#undef SHIM_PTR
#undef SHIM_OBJ
#undef SHIM_VEC
#undef SHIM_VEC_ID
#undef SHIM_VEC_STR

}
