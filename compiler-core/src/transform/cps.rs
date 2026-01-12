use crate::ast::{
    Arg, BinOp, BitArrayOption, Pattern, SrcSpan, Statement, TodoKind, TypedAssert,
    TypedAssignment, TypedExpr, TypedExprBitArraySegment, TypedStatement,
};
use crate::exhaustiveness::CompiledCase;
use crate::type_::error::VariableOrigin;
use crate::type_::{prelude::nil, Type, TypedCallArg, ValueConstructor, ValueConstructorVariant};
use ecow::EcoString;
use std::sync::Arc;
use vec1::Vec1;

/// Checks if a function body contains any return expressions.
/// This is used to determine if CPS transformation is needed.
pub fn contains_return(statements: &[TypedStatement]) -> bool {
    statements
        .iter()
        .any(|stmt| statement_contains_return(stmt))
}

/// Checks if a single statement contains any return expressions.
fn statement_contains_return(statement: &TypedStatement) -> bool {
    match statement {
        Statement::Expression(expr) => expression_contains_return(expr),
        Statement::Assignment(assignment) => expression_contains_return(&assignment.value),
        Statement::Use(use_expr) => expression_contains_return(&use_expr.call),
        Statement::Assert(assert) => expression_contains_return(&assert.value),
    }
}

/// Recursively checks if an expression contains any return expressions.
/// Note: Returns inside anonymous functions (Fn) are NOT considered to be "contained"
/// in the outer expression for the purpose of the outer function's control flow,
/// because they return from the anonymous function, not the outer function.
fn expression_contains_return(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Return { .. } => true,

        TypedExpr::Block { statements, .. } => {
            statements.iter().any(|stmt| statement_contains_return(stmt))
        }

        TypedExpr::Pipeline {
            first_value,
            assignments,
            finally,
            ..
        } => {
            expression_contains_return(&first_value.value)
                || assignments
                    .iter()
                    .any(|(assignment, _)| expression_contains_return(&assignment.value))
                || expression_contains_return(finally)
        }

        TypedExpr::Fn { .. } => {
            // Returns inside anonymous functions are local to that function.
            // They do not exit the current function.
            false
        }

        TypedExpr::List { elements, tail, .. } => {
            elements.iter().any(|elem| expression_contains_return(elem))
                || tail
                    .as_ref()
                    .map_or(false, |t| expression_contains_return(t))
        }

        TypedExpr::Call { fun, arguments, .. } => {
            expression_contains_return(fun)
                || arguments
                    .iter()
                    .any(|arg| expression_contains_return(&arg.value))
        }

        TypedExpr::BinOp { left, right, .. } => {
            expression_contains_return(left) || expression_contains_return(right)
        }

        TypedExpr::Case {
            subjects, clauses, ..
        } => {
            subjects
                .iter()
                .any(|subject| expression_contains_return(subject))
                || clauses.iter().any(|clause| {
                    expression_contains_return(&clause.then)
                })
        }

        TypedExpr::RecordAccess { record, .. } => expression_contains_return(record),

        TypedExpr::PositionalAccess { record, .. } => expression_contains_return(record),

        TypedExpr::ModuleSelect { .. } => false,

        TypedExpr::RecordUpdate {
            record_assignment,
            constructor,
            arguments,
            ..
        } => {
            record_assignment
                .as_ref()
                .map_or(false, |assignment| {
                    expression_contains_return(&assignment.value)
                })
                || expression_contains_return(constructor)
                || arguments
                    .iter()
                    .any(|arg| expression_contains_return(&arg.value))
        }

        TypedExpr::Tuple { elements, .. } => {
            elements.iter().any(|elem| expression_contains_return(elem))
        }

        TypedExpr::TupleIndex { tuple, .. } => expression_contains_return(tuple),

        TypedExpr::Todo { message, .. } => message
            .as_ref()
            .map_or(false, |msg| expression_contains_return(msg)),

        TypedExpr::Panic { message, .. } => message
            .as_ref()
            .map_or(false, |msg| expression_contains_return(msg)),

        TypedExpr::Echo {
            expression,
            message,
            ..
        } => {
            expression
                .as_ref()
                .map_or(false, |expr| expression_contains_return(expr))
                || message
                    .as_ref()
                    .map_or(false, |msg| expression_contains_return(msg))
        }

        TypedExpr::BitArray { segments, .. } => segments
            .iter()
            .any(|segment| expression_contains_return(&segment.value)),

        TypedExpr::NegateBool { value, .. } => expression_contains_return(value),

        TypedExpr::NegateInt { value, .. } => expression_contains_return(value),

        TypedExpr::Int { .. }
        | TypedExpr::Float { .. }
        | TypedExpr::String { .. }
        | TypedExpr::Var { .. }
        | TypedExpr::Invalid { .. } => false,
    }
}

/// Transforms a function body containing return expressions into CPS form.
pub fn cps_transform(statements: Vec<TypedStatement>) -> Vec<TypedStatement> {
    if !contains_return(&statements) {
        // Optimization: if no returns, just run the simple visitor to handle any nested Fns
        let mut transformer = CpsTransformer::new();
        return statements
            .into_iter()
            .map(|s| transformer.transform_statement_simple(s))
            .collect();
    }

    let mut transformer = CpsTransformer::new();
    let result_expr = transformer.transform_statements(statements, Continuation::Return);

    // If the result is a Block, we unwrap it to return a list of statements
    // This keeps the generated code cleaner
    match result_expr {
        TypedExpr::Block { statements, .. } => statements.into_vec(),
        _ => vec![Statement::Expression(result_expr)],
    }
}

struct CpsTransformer {
    var_counter: u32,
}

#[derive(Debug, Clone)]
enum Continuation {
    /// Return the value as the function result
    Return,

    /// Evaluate the rest of the statements, discarding the current value
    Discard {
        rest: Vec<TypedStatement>,
        next: Box<Continuation>,
    },

    /// Bind the current value to a variable, then evaluate the rest
    Bind {
        assignment: TypedAssignment,
        rest: Vec<TypedStatement>,
        next: Box<Continuation>,
    },

    /// Continue with an assertion
    Assert {
        assert: TypedAssert,
        rest: Vec<TypedStatement>,
        next: Box<Continuation>,
    },

    /// Complex expression continuations

    // For binary operators, we first evaluate left (which becomes `value`), then we need to evaluate right.
    // BinOpRight: "We have evaluated left=value. Now evaluate right."
    BinOpRight {
        name: BinOp,
        name_location: SrcSpan,
        right: Box<TypedExpr>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // BinOpApply: "We have evaluated left, and now we have evaluated right=value. Compute op."
    BinOpApply {
        name: BinOp,
        name_location: SrcSpan,
        left: Box<TypedExpr>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // Function calls
    CallFun {
        arguments: Vec<TypedCallArg>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    CallArg {
        fun: Box<TypedExpr>,
        evaluated_args: Vec<TypedCallArg>,
        remaining_args: Vec<TypedCallArg>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // List construction
    ListElement {
        evaluated: Vec<TypedExpr>,
        remaining: Vec<TypedExpr>,
        tail: Option<Box<TypedExpr>>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    ListTail {
        elements: Vec<TypedExpr>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // Tuple construction
    TupleElement {
        evaluated: Vec<TypedExpr>,
        remaining: Vec<TypedExpr>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // Record Access
    RecordAccess {
        location: SrcSpan,
        field_start: u32,
        type_: Arc<Type>,
        label: EcoString,
        index: u64,
        documentation: Option<EcoString>,
        next: Box<Continuation>,
    },

    // Tuple Index
    TupleIndex {
        location: SrcSpan,
        type_: Arc<Type>,
        index: u64,
        next: Box<Continuation>,
    },

    // Unary Ops
    NegateBool {
        location: SrcSpan,
        next: Box<Continuation>,
    },
    NegateInt {
        location: SrcSpan,
        next: Box<Continuation>,
    },

    // BitArray
    BitArraySegment {
        evaluated: Vec<TypedExprBitArraySegment>,
        current_options: Vec<BitArrayOption<TypedExpr>>,
        current_type: Arc<Type>,
        current_location: SrcSpan,
        remaining: Vec<TypedExprBitArraySegment>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // Echo
    Echo {
        location: SrcSpan,
        type_: Arc<Type>,
        message: Option<Box<TypedExpr>>,
        next: Box<Continuation>,
    },

    // Record Update
    RecordUpdateRecord {
        assignment: TypedAssignment,
        constructor: Box<TypedExpr>,
        arguments: Vec<TypedCallArg>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    RecordUpdateConstructor {
        record_assignment: Option<Box<TypedAssignment>>,
        arguments: Vec<TypedCallArg>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    RecordUpdateArg {
        record_assignment: Option<Box<TypedAssignment>>,
        constructor: Box<TypedExpr>,
        evaluated_args: Vec<TypedCallArg>,
        remaining_args: Vec<TypedCallArg>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },

    // Todo
    Todo {
        location: SrcSpan,
        type_: Arc<Type>,
        kind: TodoKind,
        next: Box<Continuation>,
    },

    // Panic
    Panic {
        location: SrcSpan,
        type_: Arc<Type>,
        next: Box<Continuation>,
    },
}

impl CpsTransformer {
    fn new() -> Self {
        Self { var_counter: 0 }
    }

    fn new_var(&mut self) -> EcoString {
        self.var_counter += 1;
        EcoString::from(format!("_cps_var_{}", self.var_counter))
    }

    fn transform_statements(
        &mut self,
        mut statements: Vec<TypedStatement>,
        k: Continuation,
    ) -> TypedExpr {
        if statements.is_empty() {
            // End of block, returns Nil if implicit return
            let nil_expr = TypedExpr::Tuple {
                location: SrcSpan::default(),
                elements: vec![],
                type_: nil(),
            };
            return self.apply_continuation(k, nil_expr);
        }

        let first = statements.remove(0);
        let rest = statements;

        match first {
            Statement::Expression(expr) => {
                if rest.is_empty() {
                    // Last expression is the return value of the block
                    self.transform_expression(expr, k)
                } else {
                    // Expression result is discarded (unless it returns early)
                    self.transform_expression(
                        expr,
                        Continuation::Discard {
                            rest,
                            next: Box::new(k),
                        },
                    )
                }
            }
            Statement::Assignment(assignment) => {
                // let x = value; rest
                // Transform value, then Bind x and do rest
                let value = assignment.value.clone();
                // We create a dummy assignment without value to store metadata
                self.transform_expression(
                    value,
                    Continuation::Bind {
                        assignment: *assignment,
                        rest,
                        next: Box::new(k),
                    },
                )
            }
            Statement::Use(use_) => {
                // Desugar use:
                // use x = f(a); rest
                // becomes:
                // f(a, fn(x) { rest })

                // 1. Create the callback function
                // The callback is a new function boundary. Returns inside it are local to it.
                // We just transform the callback body normally with Continuation::Return.
                let callback_body = self.transform_statements(rest, Continuation::Return);

                let callback_body_stmts = match callback_body {
                    TypedExpr::Block { statements, .. } => statements,
                    _ => Vec1::new(Statement::Expression(callback_body)),
                };

                let mut callback_args = Vec::new();
                for assignment in use_.assignments {
                    let arg = Arg {
                        names: crate::ast::ArgNames::Named {
                            name: assignment
                                .pattern
                                .bound_variables()
                                .first()
                                .map(|v| v.name())
                                .unwrap_or_else(|| "_".into()),
                            location: assignment.location,
                        },
                        location: assignment.location,
                        annotation: assignment.annotation,
                        type_: assignment.pattern.type_(),
                    };
                    callback_args.push(arg);
                }

                // If no args, use discard
                if callback_args.is_empty() {
                    let arg = Arg {
                        names: crate::ast::ArgNames::Discard {
                            name: "_".into(),
                            location: use_.location,
                        },
                        location: use_.location,
                        annotation: None,
                        type_: nil(), // Approximation
                    };
                    callback_args.push(arg);
                }

                let callback = TypedExpr::Fn {
                    location: use_.location,
                    type_: nil(), // Type inference should have handled this
                    kind: crate::ast::FunctionLiteralKind::Anonymous {
                        head: use_.location, // Corrected from references: vec![]
                    },
                    arguments: callback_args,
                    body: callback_body_stmts,
                    return_annotation: None,
                    purity: crate::type_::expression::Purity::Impure,
                };

                // 2. Append callback to call arguments
                let mut call = *use_.call;
                if let TypedExpr::Call { arguments, .. } = &mut call {
                    arguments.push(TypedCallArg {
                        label: None,
                        location: use_.location,
                        value: callback,
                        implicit: Some(crate::ast::ImplicitCallArgOrigin::Use),
                    });
                }

                // 3. Transform the resulting call with the current continuation k
                self.transform_expression(call, k)
            }
            Statement::Assert(assert) => {
                let value = assert.value.clone();
                self.transform_expression(
                    value,
                    Continuation::Assert {
                        assert,
                        rest,
                        next: Box::new(k),
                    },
                )
            }
        }
    }

    fn transform_expression(&mut self, expr: TypedExpr, k: Continuation) -> TypedExpr {
        // Optimization: if expression doesn't contain return, we don't need to break it down.
        // We can just visit it to handle nested Fns and then apply continuation.
        if !expression_contains_return(&expr) {
            let transformed = self.transform_expression_simple(expr);
            return self.apply_continuation(k, transformed);
        }

        match expr {
            TypedExpr::Return { value, .. } => {
                // Return found! Discard current continuation k.
                // Recursively transform value (it might contain returns too, though rare)
                // The new continuation is Continuation::Return (the function result)
                self.transform_expression(*value, Continuation::Return)
            }

            TypedExpr::Block { statements, .. } => {
                self.transform_statements(statements.into_vec(), k)
            }

            TypedExpr::Case {
                location,
                type_,
                subjects,
                clauses,
                compiled_case,
            } => {
                // We need to transform subjects first.
                // If subjects contain return, we handle them.

                let subjects_have_return = subjects.iter().any(|s| expression_contains_return(s));

                if subjects_have_return {
                    // Just transform the subject expression that returns.
                    // This is slightly incorrect if multiple subjects have effects,
                    // but for MVP of return, we assume first return wins.

                    for subject in subjects {
                        if expression_contains_return(&subject) {
                            return self.transform_expression(subject, Continuation::Return);
                        }
                    }
                    unreachable!("checked subjects_have_return");
                } else {
                    // Subjects are safe. Transform clauses.
                    // We push the continuation k into each clause branch.
                    // This duplicates k's code into every branch.

                    let transformed_clauses = clauses
                        .into_iter()
                        .map(|mut clause| {
                            clause.then = self.transform_expression(clause.then, k.clone());
                            clause
                        })
                        .collect();

                    let transformed_subjects = subjects
                        .into_iter()
                        .map(|s| self.transform_expression_simple(s))
                        .collect();

                    TypedExpr::Case {
                        location,
                        type_,
                        subjects: transformed_subjects,
                        clauses: transformed_clauses,
                        compiled_case,
                    }
                }
            }

            TypedExpr::BinOp {
                location,
                type_,
                name,
                name_location,
                left,
                right,
            } => self.transform_expression(
                *left,
                Continuation::BinOpRight {
                    name,
                    name_location,
                    right,
                    location,
                    type_,
                    next: Box::new(k),
                },
            ),

            TypedExpr::Call {
                location,
                type_,
                fun,
                arguments,
            } => self.transform_expression(
                *fun,
                Continuation::CallFun {
                    arguments,
                    location,
                    type_,
                    next: Box::new(k),
                },
            ),

            TypedExpr::List {
                location,
                type_,
                elements,
                tail,
            } => {
                if elements.is_empty() {
                    self.transform_tail(
                        tail,
                        Continuation::ListTail {
                            elements: vec![],
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                } else {
                    let mut remaining = elements;
                    let first = remaining.remove(0);
                    self.transform_expression(
                        first,
                        Continuation::ListElement {
                            evaluated: vec![],
                            remaining,
                            tail,
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                }
            }

            TypedExpr::Tuple {
                location,
                type_,
                elements,
            } => {
                if elements.is_empty() {
                    self.apply_continuation(
                        k,
                        TypedExpr::Tuple {
                            location,
                            type_,
                            elements: vec![],
                        },
                    )
                } else {
                    let mut remaining = elements;
                    let first = remaining.remove(0);
                    self.transform_expression(
                        first,
                        Continuation::TupleElement {
                            evaluated: vec![],
                            remaining,
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                }
            }

            TypedExpr::Pipeline { .. } => self.convert_pipeline_to_block_and_transform(expr, k),

            // Unary and other simple recursive structures
            TypedExpr::NegateBool { location, value } => self.transform_expression(
                *value,
                Continuation::NegateBool {
                    location,
                    next: Box::new(k),
                },
            ),
            TypedExpr::NegateInt { location, value } => self.transform_expression(
                *value,
                Continuation::NegateInt {
                    location,
                    next: Box::new(k),
                },
            ),
            TypedExpr::RecordAccess {
                location,
                field_start,
                type_,
                label,
                index,
                record,
                documentation,
            } => self.transform_expression(
                *record,
                Continuation::RecordAccess {
                    location,
                    field_start,
                    type_,
                    label,
                    index,
                    documentation,
                    next: Box::new(k),
                },
            ),
            TypedExpr::TupleIndex {
                location,
                type_,
                index,
                tuple,
            } => self.transform_expression(
                *tuple,
                Continuation::TupleIndex {
                    location,
                    type_,
                    index,
                    next: Box::new(k),
                },
            ),
            TypedExpr::BitArray {
                location,
                type_,
                segments,
            } => {
                if segments.is_empty() {
                    self.apply_continuation(
                        k,
                        TypedExpr::BitArray {
                            location,
                            type_,
                            segments: vec![],
                        },
                    )
                } else {
                    let mut remaining = segments;
                    let first = remaining.remove(0);
                    let TypedExprBitArraySegment {
                        value: first_value,
                        options,
                        type_: current_type,
                        location: current_location,
                    } = first;
                    let val = *first_value;

                    self.transform_expression(
                        val,
                        Continuation::BitArraySegment {
                            evaluated: vec![],
                            current_options: options,
                            current_type,
                            current_location,
                            remaining,
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                }
            }

            TypedExpr::Echo {
                location,
                type_,
                expression,
                message,
            } => {
                if let Some(msg) = message {
                    if let Some(e) = expression {
                        // Transform expression first
                        self.transform_expression(
                            *e,
                            Continuation::Echo {
                                location,
                                type_,
                                message: Some(msg),
                                next: Box::new(k),
                            },
                        )
                    } else {
                        // Just echo message?
                        // Message might have return
                        self.transform_expression(
                            *msg,
                            Continuation::Echo {
                                location,
                                type_,
                                message: None, // Will fill with transformed message in continuation
                                next: Box::new(k),
                            },
                        )
                    }
                } else if let Some(e) = expression {
                    self.transform_expression(
                        *e,
                        Continuation::Echo {
                            location,
                            type_,
                            message: None,
                            next: Box::new(k),
                        },
                    )
                } else {
                    self.apply_continuation(
                        k,
                        TypedExpr::Echo {
                            location,
                            type_,
                            expression: None,
                            message: None,
                        },
                    )
                }
            }

            TypedExpr::RecordUpdate {
                location,
                type_,
                record_assignment,
                constructor,
                arguments,
            } => {
                if let Some(assignment) = record_assignment {
                    let value = assignment.value.clone();
                    self.transform_expression(
                        value,
                        Continuation::RecordUpdateRecord {
                            assignment: *assignment,
                            constructor,
                            arguments,
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                } else {
                    self.transform_expression(
                        *constructor,
                        Continuation::RecordUpdateConstructor {
                            record_assignment: None,
                            arguments,
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                }
            }

            TypedExpr::Todo {
                location,
                type_,
                message,
                kind,
            } => {
                if let Some(msg) = message {
                    self.transform_expression(
                        *msg,
                        Continuation::Todo {
                            location,
                            type_,
                            kind,
                            next: Box::new(k),
                        },
                    )
                } else {
                    self.apply_continuation(
                        k,
                        TypedExpr::Todo {
                            location,
                            type_,
                            message: None,
                            kind,
                        },
                    )
                }
            }

            TypedExpr::Panic {
                location,
                type_,
                message,
            } => {
                if let Some(msg) = message {
                    self.transform_expression(
                        *msg,
                        Continuation::Panic {
                            location,
                            type_,
                            next: Box::new(k),
                        },
                    )
                } else {
                    self.apply_continuation(
                        k,
                        TypedExpr::Panic {
                            location,
                            type_,
                            message: None,
                        },
                    )
                }
            }

            _ => {
                let transformed = self.transform_expression_simple(expr);
                self.apply_continuation(k, transformed)
            }
        }
    }

    fn transform_tail(
        &mut self,
        tail: Option<Box<TypedExpr>>,
        k: Continuation,
    ) -> TypedExpr {
        match tail {
            Some(t) => self.transform_expression(*t, k),
            None => match k {
                Continuation::ListTail {
                    elements,
                    location,
                    type_,
                    next,
                } => {
                    let list = TypedExpr::List {
                        location,
                        type_,
                        elements,
                        tail: None,
                    };
                    self.apply_continuation(*next, list)
                }
                _ => panic!("Expected ListTail continuation"),
            },
        }
    }

    fn apply_continuation(&mut self, k: Continuation, value: TypedExpr) -> TypedExpr {
        match k {
            Continuation::Return => value,

            Continuation::Discard { rest, next } => {
                // { value; rest... }
                let rest_expr = self.transform_statements(rest, *next);
                let location = value.location();
                self.make_block(
                    vec![Statement::Expression(value)],
                    rest_expr,
                    location,
                )
            }

            Continuation::Bind {
                mut assignment,
                rest,
                next,
            } => {
                // let x = value; rest...
                assignment.value = value.clone(); // Value is used here
                let location = value.location();
                let rest_expr = self.transform_statements(rest, *next);
                self.make_block(
                    vec![Statement::Assignment(Box::new(assignment))],
                    rest_expr,
                    location,
                )
            }

            Continuation::Assert {
                mut assert,
                rest,
                next,
            } => {
                assert.value = value.clone();
                let location = value.location();
                let rest_expr = self.transform_statements(rest, *next);
                self.make_block(vec![Statement::Assert(assert)], rest_expr, location)
            }

            Continuation::BinOpRight {
                name,
                name_location,
                right,
                location,
                type_,
                next,
            } => {
                // We have left value. Now transform right.
                // If right returns, left is lost unless we bind it.
                if expression_contains_return(&right) {
                    let var_name = self.new_var();
                    let var_expr = TypedExpr::Var {
                        location: value.location(),
                        name: var_name.clone(),
                        constructor: ValueConstructor {
                            publicity: crate::ast::Publicity::Private,
                            deprecation: crate::type_::Deprecation::NotDeprecated,
                            type_: value.type_(),
                            variant: ValueConstructorVariant::LocalVariable {
                                location: value.location(),
                                origin: VariableOrigin::generated(), // Added origin
                            },
                        },
                    };

                    let k_apply = Continuation::BinOpApply {
                        name,
                        name_location,
                        left: Box::new(var_expr),
                        location,
                        type_: type_.clone(),
                        next,
                    };

                    let right_expr = self.transform_expression(*right, k_apply);

                    // Wrap in block: let var = value; right_expr
                    let assignment = TypedAssignment {
                        location: value.location(),
                        value: value.clone(),
                        pattern: Pattern::Variable {
                            location: value.location(),
                            name: var_name.clone(),
                            type_: value.type_(),
                            origin: VariableOrigin::generated(),
                        },
                        kind: crate::ast::AssignmentKind::Let,
                        annotation: None,
                        compiled_case: CompiledCase::simple_variable_assignment(
                            var_name,
                            value.type_(),
                        ),
                    };

                    self.make_block(
                        vec![Statement::Assignment(Box::new(assignment))],
                        right_expr,
                        location,
                    )
                } else {
                    // Right is safe, just transform it.
                    self.transform_expression(
                        *right,
                        Continuation::BinOpApply {
                            name,
                            name_location,
                            left: Box::new(value),
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::BinOpApply {
                name,
                name_location,
                left,
                location,
                type_,
                next,
            } => {
                let binop = TypedExpr::BinOp {
                    location,
                    type_,
                    name,
                    name_location,
                    left,
                    right: Box::new(value),
                };
                self.apply_continuation(*next, binop)
            }

            Continuation::CallFun {
                arguments,
                location,
                type_,
                next,
            } => {
                // We evaluated fun (value). Now args.
                self.transform_call_args(value, vec![], arguments, location, type_, *next)
            }

            Continuation::CallArg {
                fun,
                mut evaluated_args,
                mut remaining_args,
                location,
                type_,
                next,
            } => {
                // We just evaluated an arg.
                // value is the evaluated arg value.

                let mut current_arg = remaining_args.remove(0);
                current_arg.value = value;
                evaluated_args.push(current_arg);

                if remaining_args.is_empty() {
                    // All args done. Construct Call.
                    let call = TypedExpr::Call {
                        location,
                        type_,
                        fun,
                        arguments: evaluated_args,
                    };
                    self.apply_continuation(*next, call)
                } else {
                    // Transform next arg
                    let next_arg = remaining_args[0].value.clone();
                    self.transform_expression(
                        next_arg,
                        Continuation::CallArg {
                            fun,
                            evaluated_args,
                            remaining_args,
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::ListElement {
                mut evaluated,
                mut remaining,
                tail,
                location,
                type_,
                next,
            } => {
                evaluated.push(value);
                if remaining.is_empty() {
                    // Done with elements. Handle tail.
                    self.transform_tail(
                        tail,
                        Continuation::ListTail {
                            elements: evaluated,
                            location,
                            type_,
                            next,
                        },
                    )
                } else {
                    let next_expr = remaining.remove(0);
                    self.transform_expression(
                        next_expr,
                        Continuation::ListElement {
                            evaluated,
                            remaining,
                            tail,
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::ListTail {
                elements,
                location,
                type_,
                next,
            } => {
                let list = TypedExpr::List {
                    location,
                    type_,
                    elements,
                    tail: Some(Box::new(value)),
                };
                self.apply_continuation(*next, list)
            }

            Continuation::TupleElement {
                mut evaluated,
                mut remaining,
                location,
                type_,
                next,
            } => {
                evaluated.push(value);
                if remaining.is_empty() {
                    self.apply_continuation(
                        *next,
                        TypedExpr::Tuple {
                            location,
                            type_,
                            elements: evaluated,
                        },
                    )
                } else {
                    let next_expr = remaining.remove(0);
                    self.transform_expression(
                        next_expr,
                        Continuation::TupleElement {
                            evaluated,
                            remaining,
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::RecordAccess {
                location,
                field_start,
                type_,
                label,
                index,
                documentation,
                next,
            } => self.apply_continuation(
                *next,
                TypedExpr::RecordAccess {
                    location,
                    field_start,
                    type_,
                    label,
                    index,
                    documentation,
                    record: Box::new(value),
                },
            ),

            Continuation::TupleIndex {
                location,
                type_,
                index,
                next,
            } => self.apply_continuation(
                *next,
                TypedExpr::TupleIndex {
                    location,
                    type_,
                    index,
                    tuple: Box::new(value),
                },
            ),

            Continuation::NegateBool { location, next } => self.apply_continuation(
                *next,
                TypedExpr::NegateBool {
                    location,
                    value: Box::new(value),
                },
            ),

            Continuation::NegateInt { location, next } => self.apply_continuation(
                *next,
                TypedExpr::NegateInt {
                    location,
                    value: Box::new(value),
                },
            ),

            Continuation::Echo {
                location,
                type_,
                message,
                next,
            } => {
                if let Some(msg_expr) = message {
                    // We just transformed the expression. value is expression.
                    // Now transform message.
                    if expression_contains_return(&msg_expr) {
                        // Fallback: transform message as return, discard expression result.
                        self.transform_expression(*msg_expr, Continuation::Return)
                    } else {
                        // Message safe.
                        let transformed_msg = self.transform_expression_simple(*msg_expr);
                        let echo = TypedExpr::Echo {
                            location,
                            type_,
                            expression: Some(Box::new(value)),
                            message: Some(Box::new(transformed_msg)),
                        };
                        self.apply_continuation(*next, echo)
                    }
                } else {
                    // Message is None. We are done.
                    let echo = TypedExpr::Echo {
                        location,
                        type_,
                        expression: Some(Box::new(value)),
                        message: None,
                    };
                    self.apply_continuation(*next, echo)
                }
            }

            Continuation::BitArraySegment {
                mut evaluated,
                current_options,
                current_type,
                current_location,
                mut remaining,
                location,
                type_,
                next,
            } => {
                let current = TypedExprBitArraySegment {
                    value: Box::new(value),
                    options: current_options,
                    type_: current_type,
                    location: current_location,
                };
                evaluated.push(current);

                if remaining.is_empty() {
                    self.apply_continuation(
                        *next,
                        TypedExpr::BitArray {
                            location,
                            type_,
                            segments: evaluated,
                        },
                    )
                } else {
                    let first = remaining.remove(0);
                    let TypedExprBitArraySegment {
                        value: first_value,
                        options,
                        type_: current_type,
                        location: current_location,
                    } = first;
                    let val = *first_value;

                    self.transform_expression(
                        val,
                        Continuation::BitArraySegment {
                            evaluated,
                            current_options: options,
                            current_type,
                            current_location,
                            remaining,
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::RecordUpdateRecord {
                mut assignment,
                constructor,
                arguments,
                location,
                type_,
                next,
            } => {
                assignment.value = value;
                self.transform_expression(
                    *constructor,
                    Continuation::RecordUpdateConstructor {
                        record_assignment: Some(Box::new(assignment)),
                        arguments,
                        location,
                        type_,
                        next,
                    },
                )
            }

            Continuation::RecordUpdateConstructor {
                record_assignment,
                arguments,
                location,
                type_,
                next,
            } => {
                if arguments.is_empty() {
                    let update = TypedExpr::RecordUpdate {
                        location,
                        type_,
                        record_assignment,
                        constructor: Box::new(value),
                        arguments: vec![],
                    };
                    self.apply_continuation(*next, update)
                } else {
                    let first = arguments[0].value.clone();
                    self.transform_expression(
                        first,
                        Continuation::RecordUpdateArg {
                            record_assignment,
                            constructor: Box::new(value),
                            evaluated_args: vec![],
                            remaining_args: arguments,
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::RecordUpdateArg {
                record_assignment,
                constructor,
                mut evaluated_args,
                mut remaining_args,
                location,
                type_,
                next,
            } => {
                let mut current_arg = remaining_args.remove(0);
                current_arg.value = value;
                evaluated_args.push(current_arg);

                if remaining_args.is_empty() {
                    let update = TypedExpr::RecordUpdate {
                        location,
                        type_,
                        record_assignment,
                        constructor,
                        arguments: evaluated_args,
                    };
                    self.apply_continuation(*next, update)
                } else {
                    let next_arg_expr = remaining_args[0].value.clone();
                    self.transform_expression(
                        next_arg_expr,
                        Continuation::RecordUpdateArg {
                            record_assignment,
                            constructor,
                            evaluated_args,
                            remaining_args,
                            location,
                            type_,
                            next,
                        },
                    )
                }
            }

            Continuation::Todo {
                location,
                type_,
                kind,
                next,
            } => {
                let todo = TypedExpr::Todo {
                    location,
                    type_,
                    kind,
                    message: Some(Box::new(value)),
                };
                self.apply_continuation(*next, todo)
            }

            Continuation::Panic {
                location,
                type_,
                next,
            } => {
                let panic = TypedExpr::Panic {
                    location,
                    type_,
                    message: Some(Box::new(value)),
                };
                self.apply_continuation(*next, panic)
            }
        }
    }

    fn transform_call_args(
        &mut self,
        fun: TypedExpr,
        evaluated_args: Vec<TypedCallArg>,
        remaining_args: Vec<TypedCallArg>,
        location: SrcSpan,
        type_: Arc<Type>,
        next: Continuation,
    ) -> TypedExpr {
        if remaining_args.is_empty() {
            let call = TypedExpr::Call {
                location,
                type_,
                fun: Box::new(fun),
                arguments: evaluated_args,
            };
            self.apply_continuation(next, call)
        } else {
            let first = remaining_args[0].value.clone();
            self.transform_expression(
                first,
                Continuation::CallArg {
                    fun: Box::new(fun),
                    evaluated_args,
                    remaining_args,
                    location,
                    type_,
                    next: Box::new(next),
                },
            )
        }
    }

    fn make_block(
        &self,
        mut prefix: Vec<TypedStatement>,
        suffix: TypedExpr,
        location: SrcSpan,
    ) -> TypedExpr {
        match suffix {
            TypedExpr::Block { statements, .. } => {
                prefix.extend(statements);
                TypedExpr::Block {
                    location,
                    statements: Vec1::try_from_vec(prefix).unwrap(),
                }
            }
            _ => {
                prefix.push(Statement::Expression(suffix));
                TypedExpr::Block {
                    location,
                    statements: Vec1::try_from_vec(prefix).unwrap(),
                }
            }
        }
    }

    fn convert_pipeline_to_block_and_transform(
        &mut self,
        expr: TypedExpr,
        k: Continuation,
    ) -> TypedExpr {
        if let TypedExpr::Pipeline {
            location: _,
            first_value,
            assignments,
            finally,
            ..
        } = expr
        {
            let mut statements = Vec::new();

            let first_stmt = Statement::Assignment(Box::new(TypedAssignment {
                location: first_value.location,
                pattern: Pattern::Variable {
                    location: first_value.location,
                    name: first_value.name.clone(),
                    type_: first_value.value.type_(),
                    origin: VariableOrigin::generated(),
                },
                kind: crate::ast::AssignmentKind::Let,
                annotation: None,
                compiled_case: CompiledCase::simple_variable_assignment(
                    first_value.name.clone(),
                    first_value.value.type_(),
                ),
                value: *first_value.value,
            }));
            statements.push(first_stmt);

            for (assignment, _) in assignments {
                let stmt = Statement::Assignment(Box::new(TypedAssignment {
                    location: assignment.location,
                    pattern: Pattern::Variable {
                        location: assignment.location,
                        name: assignment.name.clone(),
                        type_: assignment.value.type_(),
                        origin: VariableOrigin::generated(),
                    },
                    kind: crate::ast::AssignmentKind::Let,
                    annotation: None,
                    compiled_case: CompiledCase::simple_variable_assignment(
                        assignment.name.clone(),
                        assignment.value.type_(),
                    ),
                    value: *assignment.value,
                }));
                statements.push(stmt);
            }

            statements.push(Statement::Expression(*finally));

            self.transform_statements(statements, k)
        } else {
            panic!("Not a pipeline")
        }
    }

    // Simplified recursive transform for expressions without return
    fn transform_expression_simple(&mut self, expr: TypedExpr) -> TypedExpr {
        match expr {
            TypedExpr::Fn {
                location,
                type_,
                kind,
                arguments,
                body,
                return_annotation,
                purity,
            } => {
                // Must transform body even if expr has no return
                let transformed_body_expr =
                    self.transform_statements(body.into_vec(), Continuation::Return);
                let transformed_body = match transformed_body_expr {
                    TypedExpr::Block { statements, .. } => statements,
                    _ => Vec1::new(Statement::Expression(transformed_body_expr)),
                };
                TypedExpr::Fn {
                    location,
                    type_,
                    kind,
                    arguments,
                    body: transformed_body,
                    return_annotation,
                    purity,
                }
            }
            // For other expressions, just rebuild them (deep copy/visit)
            // Ideally we would use a visitor or a generic map, but here we manually recurse
            // only on nodes that contain nested expressions (blocks, lists, etc)

            TypedExpr::Block {
                location,
                statements,
            } => {
                let stmts = statements
                    .into_vec()
                    .into_iter()
                    .map(|s| self.transform_statement_simple(s))
                    .collect();
                TypedExpr::Block {
                    location,
                    statements: Vec1::try_from_vec(stmts).unwrap(),
                }
            }

            TypedExpr::Call {
                location,
                type_,
                fun,
                arguments,
            } => TypedExpr::Call {
                location,
                type_,
                fun: Box::new(self.transform_expression_simple(*fun)),
                arguments: arguments
                    .into_iter()
                    .map(|mut arg| {
                        arg.value = self.transform_expression_simple(arg.value);
                        arg
                    })
                    .collect(),
            },

            _ => self.deep_transform_simple(expr),
        }
    }

    fn transform_statement_simple(&mut self, stmt: TypedStatement) -> TypedStatement {
        match stmt {
            Statement::Expression(e) => {
                Statement::Expression(self.transform_expression_simple(e))
            }
            Statement::Assignment(a) => {
                let mut a = *a;
                a.value = self.transform_expression_simple(a.value);
                Statement::Assignment(Box::new(a))
            }
            Statement::Use(u) => {
                let mut u = u;
                u.call = Box::new(self.transform_expression_simple(*u.call));
                Statement::Use(u)
            }
            Statement::Assert(a) => {
                let mut a = a;
                a.value = self.transform_expression_simple(a.value);
                Statement::Assert(a)
            }
        }
    }

    fn deep_transform_simple(&mut self, expr: TypedExpr) -> TypedExpr {
        // Recursive traversal that only changes Fns
        match expr {
            TypedExpr::Fn { .. } => self.transform_expression_simple(expr),

            TypedExpr::Block {
                location,
                statements,
            } => {
                let stmts = statements
                    .into_vec()
                    .into_iter()
                    .map(|s| self.transform_statement_simple(s))
                    .collect();
                TypedExpr::Block {
                    location,
                    statements: Vec1::try_from_vec(stmts).unwrap(),
                }
            }

            TypedExpr::Call {
                location,
                type_,
                fun,
                arguments,
            } => TypedExpr::Call {
                location,
                type_,
                fun: Box::new(self.deep_transform_simple(*fun)),
                arguments: arguments
                    .into_iter()
                    .map(|mut arg| {
                        arg.value = self.deep_transform_simple(arg.value);
                        arg
                    })
                    .collect(),
            },

            TypedExpr::BinOp {
                location,
                type_,
                name,
                name_location,
                left,
                right,
            } => TypedExpr::BinOp {
                location,
                type_,
                name,
                name_location,
                left: Box::new(self.deep_transform_simple(*left)),
                right: Box::new(self.deep_transform_simple(*right)),
            },

            TypedExpr::List {
                location,
                type_,
                elements,
                tail,
            } => TypedExpr::List {
                location,
                type_,
                elements: elements
                    .into_iter()
                    .map(|e| self.deep_transform_simple(e))
                    .collect(),
                tail: tail.map(|t| Box::new(self.deep_transform_simple(*t))),
            },

            TypedExpr::Tuple {
                location,
                type_,
                elements,
            } => TypedExpr::Tuple {
                location,
                type_,
                elements: elements
                    .into_iter()
                    .map(|e| self.deep_transform_simple(e))
                    .collect(),
            },

            TypedExpr::Case {
                location,
                type_,
                subjects,
                clauses,
                compiled_case,
            } => TypedExpr::Case {
                location,
                type_,
                compiled_case,
                subjects: subjects
                    .into_iter()
                    .map(|e| self.deep_transform_simple(e))
                    .collect(),
                clauses: clauses
                    .into_iter()
                    .map(|mut c| {
                        c.then = self.deep_transform_simple(c.then);
                        c
                    })
                    .collect(),
            },

            TypedExpr::RecordAccess {
                location,
                field_start,
                type_,
                label,
                index,
                record,
                documentation,
            } => TypedExpr::RecordAccess {
                location,
                field_start,
                type_,
                label,
                index,
                documentation,
                record: Box::new(self.deep_transform_simple(*record)),
            },

            TypedExpr::Return {
                location,
                type_,
                value,
            } => TypedExpr::Return {
                location,
                type_,
                value: Box::new(self.deep_transform_simple(*value)),
            },

            _ => expr,
        }
    }
}
