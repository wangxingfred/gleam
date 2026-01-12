# Requirements Document

## Introduction

本文档定义了在 Gleam 语言中实现类似 Rust 的 `return` 语法特性的需求，采用关键字 `$return`。
Gleam 是一个函数式编程语言，目前采用表达式导向的设计，函数的最后一个表达式自动作为返回值。
本特性旨在引入显式的 `$return` 关键字，允许在函数体的任意位置提前返回。

## Glossary

- **Parser**: Gleam 编译器中负责将源代码转换为未类型化 AST 的组件
- **Lexer**: 词法分析器，将源代码转换为 Token 流
- **AST**: 抽象语法树 (Abstract Syntax Tree)
- **UntypedExpr**: 未经类型检查的表达式 AST 节点
- **TypedExpr**: 经过类型检查的表达式 AST 节点
- **Statement**: 函数体内的语句，包括表达式、赋值、use 表达式和断言
- **Type_Checker**: 类型检查器，负责推断和验证表达式类型
- **Code_Generator**: 代码生成器，将 TypedAST 转换为目标代码 (Erlang/JavaScript)
- **Return_Expression**: 新增的 return 表达式，用于提前返回函数值
- **Control_Flow**: 控制流，程序执行的路径
- **Early_Return**: 提前返回，在函数体中间位置返回值而非等到函数末尾

## Requirements

### Requirement 1: Return 关键字词法支持

**User Story:** As a Gleam developer, I want the compiler to recognize `$return` as a keyword, so that I can use it in my code.

#### Acceptance Criteria

1. THE Lexer SHALL recognize `$return` as a reserved keyword token
2. WHEN the Lexer encounters the string "$return" THEN the Lexer SHALL emit a `Token::Return` token
3. THE Parser SHALL treat `$return` as a reserved word that cannot be used as a variable name

### Requirement 2: Return 表达式语法解析

**User Story:** As a Gleam developer, I want to write `$return expression` to explicitly return a value from a function, so that I can exit a function early.

#### Acceptance Criteria

1. WHEN the Parser encounters `$return` followed by an expression THEN the Parser SHALL parse it as a Return expression
2. WHEN the Parser encounters `$return` without a following expression THEN the Parser SHALL report a syntax error indicating that an expression is required
3. THE Parser SHALL create an `UntypedExpr::Return` AST node containing the mandatory return value expression
4. WHEN `$return` appears outside a function body THEN the Parser SHALL report a syntax error

### Requirement 3: Return 表达式类型检查

**User Story:** As a Gleam developer, I want the compiler to verify that my return expressions have the correct type, so that I can catch type errors at compile time.

#### Acceptance Criteria

1. THE Type_Checker SHALL infer the type of the return expression's value
2. WHEN a return expression's type does not match the function's return type THEN the Type_Checker SHALL report a type error
3. THE Type_Checker SHALL unify the return expression type with the function's declared or inferred return type
4. WHEN multiple return expressions exist in a function THEN the Type_Checker SHALL ensure all return types are compatible

### Requirement 4: Return 表达式控制流分析

**User Story:** As a Gleam developer, I want the compiler to understand that code after a return statement is unreachable, so that I can receive appropriate warnings.

#### Acceptance Criteria

1. WHEN code appears after a return expression in the same block THEN the Type_Checker SHALL emit an unreachable code warning
2. THE Type_Checker SHALL mark return expressions as "always panics" for control flow analysis (similar to `panic`)
3. WHEN a return expression is the last statement in a block THEN the Type_Checker SHALL NOT emit any warning

### Requirement 5: Erlang 代码生成

**User Story:** As a Gleam developer, I want my return expressions to compile correctly to Erlang, so that my code runs on the BEAM.

#### Acceptance Criteria

1. THE Code_Generator SHALL transform return expressions into valid Erlang code
2. WHEN generating Erlang code for a function with return expressions THEN the Code_Generator SHALL use appropriate control flow constructs
3. THE generated Erlang code SHALL preserve the semantics of early return

### Requirement 6: JavaScript 代码生成

**User Story:** As a Gleam developer, I want my return expressions to compile correctly to JavaScript, so that my code runs in browsers and Node.js.

#### Acceptance Criteria

1. THE Code_Generator SHALL transform return expressions into JavaScript `return` statements
2. WHEN generating JavaScript code for a function with return expressions THEN the Code_Generator SHALL emit `return` statements
3. THE generated JavaScript code SHALL preserve the semantics of early return

### Requirement 7: Return 在嵌套上下文中的行为

**User Story:** As a Gleam developer, I want return to work correctly in nested contexts like case expressions and blocks, so that I can use it flexibly.

#### Acceptance Criteria

1. WHEN a return expression appears inside a case branch THEN the Return_Expression SHALL return from the enclosing function, not just the case expression
2. WHEN a return expression appears inside a block expression THEN the Return_Expression SHALL return from the enclosing function
3. WHEN a return expression appears inside an anonymous function THEN the Return_Expression SHALL return from that anonymous function, not the outer function
4. WHEN a return expression appears inside a use expression callback THEN the Return_Expression SHALL return from the callback function

### Requirement 8: 格式化器支持

**User Story:** As a Gleam developer, I want the formatter to handle return expressions correctly, so that my code is consistently formatted.

#### Acceptance Criteria

1. THE Formatter SHALL format return expressions according to Gleam style guidelines
2. WHEN formatting `$return expression` THEN the Formatter SHALL preserve appropriate spacing between `$return` keyword and the expression
3. WHEN the expression is complex THEN the Formatter SHALL apply standard expression formatting rules to the return value

### Requirement 9: 语言服务器支持

**User Story:** As a Gleam developer, I want IDE features like hover and completion to work with return expressions, so that I have a good development experience.

#### Acceptance Criteria

1. THE Language_Server SHALL provide hover information for return expressions showing the return type
2. THE Language_Server SHALL provide completion suggestions after typing `$return`
3. THE Language_Server SHALL highlight return expressions appropriately for syntax highlighting

### Requirement 10: 错误消息质量

**User Story:** As a Gleam developer, I want clear error messages when I misuse return, so that I can quickly fix my code.

#### Acceptance Criteria

1. WHEN a return type mismatch occurs THEN the Type_Checker SHALL provide a clear error message indicating the expected and actual types
2. WHEN return is used outside a function THEN the Parser SHALL provide a clear error message
3. WHEN unreachable code follows a return THEN the Type_Checker SHALL provide a helpful warning message
4. WHEN `$return` is used without a following expression THEN the Parser SHALL provide a clear error message: "Expected an expression after `$return`"
