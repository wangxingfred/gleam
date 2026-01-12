# Design Document: Gleam Return Syntax Feature

## Overview

本设计文档详细分析了在 Gleam 语言中实现类似 Rust 的 `return` 语法特性的多种实现方案。通过深入分析 Gleam 编译器架构，我们提出了三种主要实现方案，并对每种方案进行了详细的优缺点评估和打分。

### 关键约束

**`return` 必须后跟表达式**：与某些允许裸 `return` 的语言不同，Gleam 的 `return` 语法要求必须提供返回值表达式。如果需要返回空值，必须显式写 `return Nil`。

```gleam
// 正确用法
return 42
return Nil
return Ok(value)

// 错误用法 - 不允许
return  // 编译错误：Expected an expression after `return`
```

**MUST NOT：Erlang实现禁止使用throw异常实现方式**：生成Erlang代码禁止使用throw方式实现，因为try throw会导致尾递归函数失去尾递归效果，同时带来运行时额外消耗

## 背景分析

### Gleam 当前架构

Gleam 编译器采用经典的多阶段编译架构：

```
源代码 → Lexer → Parser → Untyped AST → Type Checker → Typed AST → Code Generator → 目标代码
```

关键组件：
- **Lexer** (`compiler-core/src/parse/lexer.rs`): 词法分析，生成 Token 流
- **Token** (`compiler-core/src/parse/token.rs`): Token 定义，包含所有关键字
- **Parser** (`compiler-core/src/parse.rs`): 语法分析，生成 Untyped AST
- **AST** (`compiler-core/src/ast.rs`, `ast/untyped.rs`, `ast/typed.rs`): AST 节点定义
- **Type Checker** (`compiler-core/src/type_/expression.rs`): 类型推断和检查
- **Erlang Generator** (`compiler-core/src/erlang.rs`): Erlang 代码生成
- **JavaScript Generator** (`compiler-core/src/javascript.rs`): JavaScript 代码生成

### Gleam 表达式导向设计

Gleam 是表达式导向的语言，函数体是一系列 Statement，最后一个 Statement 的值作为返回值：

```gleam
pub fn example(x: Int) -> Int {
  let y = x + 1
  y * 2  // 这是返回值
}
```

当前 Statement 类型：
- `Expression`: 裸表达式
- `Assignment`: let 绑定
- `Use`: use 表达式
- `Assert`: 断言

### 类似特性参考

Gleam 已有类似的控制流表达式：
- `panic`: 立即终止程序，类型为 `Never`/任意类型
- `todo`: 标记未完成代码，类型为任意类型

这些为 `return` 的实现提供了参考模式。

---

## 方案一：表达式级 Return (Expression-Level Return)

### 设计思路

将 `return` 作为一种新的表达式类型，类似于 `panic` 和 `todo` 的实现方式。

### 架构变更

#### 1. Token 定义
```rust
// compiler-core/src/parse/token.rs
pub enum Token {
    // ... existing tokens ...
    Return,  // 新增
}
```

#### 2. AST 节点
```rust
// compiler-core/src/ast/untyped.rs
pub enum UntypedExpr {
    // ... existing variants ...
    Return {
        location: SrcSpan,
        value: Box<Self>,  // 必须有返回值表达式
    },
}

// compiler-core/src/ast/typed.rs
pub enum TypedExpr {
    // ... existing variants ...
    Return {
        location: SrcSpan,
        type_: Arc<Type>,  // 函数返回类型
        value: Box<Self>,
    },
}
```

#### 3. Parser 修改
```rust
// compiler-core/src/parse.rs
fn parse_expression_unit(&mut self, context: ExpressionUnitContext) -> Result<Option<UntypedExpr>, ParseError> {
    match self.tok0.take() {
        // ... existing cases ...
        Some((start, Token::Return, _)) => {
            self.advance();
            // return 必须后跟表达式，不允许裸 return
            let value = self.parse_expression()?
                .ok_or_else(|| ParseError {
                    error: ParseErrorType::ExpectedExpressionAfterReturn,
                    location: SrcSpan { start, end: self.tok0_end },
                })?;
            let end = value.location().end;
            Ok(Some(UntypedExpr::Return {
                location: SrcSpan { start, end },
                value: Box::new(value),
            }))
        }
        // ...
    }
}
```

需要添加新的错误类型：
```rust
// compiler-core/src/parse/error.rs
pub enum ParseErrorType {
    // ... existing variants ...
    ExpectedExpressionAfterReturn,  // return 后缺少表达式
}
```

#### 4. 类型检查
```rust
// compiler-core/src/type_/expression.rs
impl ExprTyper {
    fn infer_return(&mut self, location: SrcSpan, value: UntypedExpr) -> TypedExpr {
        // value 是必须的，Parser 已确保存在
        let typed_value = self.infer(value);
        
        // 统一返回类型与函数返回类型
        self.unify_return_type(&typed_value.type_());
        
        // 标记后续代码不可达
        self.previous_panics = true;
        
        TypedExpr::Return {
            location,
            type_: self.expected_return_type.clone(),
            value: Box::new(typed_value),
        }
    }
}
```

#### 5. Erlang 代码生成

Erlang 没有原生的 early return，需要使用 CPS 变换或异常机制：

**方案 A: 使用 throw/catch**
```erlang
% Gleam: 
% fn example(x) {
%   if x > 0 { return x }
%   x + 1
% }

% Erlang:
example(X) ->
    try
        case X > 0 of
            true -> throw({gleam_return, X});
            false -> ok
        end,
        X + 1
    catch
        throw:{gleam_return, Value} -> Value
    end.
```

**方案 B: 使用 case 重构**
```erlang
% 编译器将代码重构为 case 表达式
example(X) ->
    case X > 0 of
        true -> X;
        false -> X + 1
    end.
```

#### 6. JavaScript 代码生成
```javascript
// 直接映射到 JavaScript return
function example(x) {
    if (x > 0) { return x; }
    return x + 1;
}
```

### 优点

1. **概念简单**: 与 `panic`/`todo` 实现模式一致
2. **JavaScript 映射自然**: 直接对应 JS 的 return
3. **类型系统集成简单**: 复用现有的类型推断机制
4. **实现工作量适中**: 主要修改集中在几个文件

### 缺点

1. **Erlang 生成复杂**: 需要 throw/catch 或代码重构
2. **性能影响**: Erlang 的 throw/catch 有性能开销
3. **调试困难**: 异常机制可能影响 Erlang 调试体验
4. **语义不一致**: 在嵌套函数中的行为需要特别处理

### 评分: 7.5/10

---

## 方案二：语句级 Return (Statement-Level Return)

### 设计思路

将 `return` 作为一种新的 Statement 类型，而非表达式。这更接近传统命令式语言的设计。

### 架构变更

#### 1. Statement 扩展
```rust
// compiler-core/src/ast.rs
pub enum Statement<TypeT, ExpressionT> {
    Expression(ExpressionT),
    Assignment(Box<Assignment<TypeT, ExpressionT>>),
    Use(Use<TypeT, ExpressionT>),
    Assert(Assert<ExpressionT>),
    Return(Return<ExpressionT>),  // 新增
}

pub struct Return<ExpressionT> {
    pub location: SrcSpan,
    pub value: ExpressionT,  // 必须有返回值表达式
}
```

#### 2. Parser 修改
```rust
fn parse_statement(&mut self) -> Result<Option<UntypedStatement>, ParseError> {
    match self.tok0.as_ref() {
        Some((start, Token::Return, _)) => {
            let start = *start;
            self.advance();
            // return 必须后跟表达式
            let value = self.parse_expression()?
                .ok_or_else(|| ParseError {
                    error: ParseErrorType::ExpectedExpressionAfterReturn,
                    location: SrcSpan { start, end: self.tok0_end },
                })?;
            Ok(Some(Statement::Return(Return {
                location: SrcSpan { start, end: value.location().end },
                value,
            })))
        }
        // ... existing cases ...
    }
}
```

#### 3. 类型检查
```rust
fn infer_statement(&mut self, statement: UntypedStatement) -> TypedStatement {
    match statement {
        Statement::Return(ret) => {
            let typed_value = self.infer(ret.value);
            // 类型检查逻辑
            self.previous_panics = true;
            Statement::Return(TypedReturn { 
                location: ret.location,
                value: typed_value,
            })
        }
        // ...
    }
}
```

### 优点

1. **语义清晰**: Return 作为语句，语义更明确
2. **控制流分析简单**: 语句级别的控制流更容易分析
3. **与其他语句一致**: 与 Assignment、Use 等语句形式一致

### 缺点

1. **表达式上下文受限**: 不能在表达式位置使用 return
2. **与 Gleam 哲学冲突**: Gleam 是表达式导向的语言
3. **代码生成同样复杂**: Erlang 生成问题依然存在
4. **灵活性降低**: 不能写 `let x = if cond { return y } else { z }`

### 评分: 6.5/10

---

## 方案三：CPS 变换 (Continuation-Passing Style Transformation)

### 设计思路

在编译期将包含 `return` 的函数转换为 CPS 风格，避免运行时的控制流跳转。

### 架构变更

#### 1. AST 保持简单
```rust
pub enum UntypedExpr {
    Return {
        location: SrcSpan,
        value: Box<Self>,  // 必须有返回值表达式
    },
}
```

#### 2. 类型检查后进行 CPS 变换

在类型检查完成后，添加一个 AST 变换阶段：

```rust
// 新增: compiler-core/src/transform/cps.rs
pub fn transform_returns(function: TypedFunction) -> TypedFunction {
    if !contains_return(&function) {
        return function;
    }
    
    // 将函数体转换为 CPS 风格
    let transformed_body = cps_transform(&function.body);
    TypedFunction {
        body: transformed_body,
        ..function
    }
}
```

#### 3. CPS 变换示例

原始代码：
```gleam
fn example(x: Int) -> Int {
  if x > 0 {
    return x * 2
  }
  x + 1
}
```

变换后（概念上）：
```gleam
fn example(x: Int) -> Int {
  case x > 0 {
    True -> x * 2
    False -> x + 1
  }
}
```

更复杂的例子：
```gleam
fn complex(x: Int) -> Int {
  let y = x + 1
  if y > 10 {
    return y
  }
  let z = y * 2
  if z > 50 {
    return z
  }
  z + y
}
```

变换后：
```gleam
fn complex(x: Int) -> Int {
  let y = x + 1
  case y > 10 {
    True -> y
    False -> {
      let z = y * 2
      case z > 50 {
        True -> z
        False -> z + y
      }
    }
  }
}
```

### 优点

1. **无运行时开销**: 编译期完成变换，无性能影响
2. **Erlang 生成简单**: 变换后的代码是标准 Gleam，正常生成
3. **语义清晰**: 变换规则明确，行为可预测
4. **调试友好**: 生成的代码是正常的 Gleam 模式

### 缺点

1. **实现复杂**: CPS 变换逻辑复杂，需要处理各种边界情况
2. **编译时间增加**: 额外的 AST 变换阶段
3. **错误消息映射**: 变换后的代码错误需要映射回原始位置
4. **代码膨胀**: 嵌套的 case 可能导致生成代码变大
5. **循环中的 return**: 需要特殊处理（Gleam 目前没有循环，但未来可能有）

### 评分: 7.0/10

---

## 方案四：混合方案 (Hybrid Approach) - 推荐

### 设计思路

结合方案一和方案三的优点：
- 在 AST 层面使用表达式级 Return（方案一）
- 对于 JavaScript 目标，直接生成 return 语句
- 对于 Erlang 目标，使用 CPS 变换（方案三）

### 架构变更

#### 1. AST 定义（同方案一）
```rust
pub enum UntypedExpr {
    Return {
        location: SrcSpan,
        value: Box<Self>,  // 必须有返回值表达式
    },
}

pub enum TypedExpr {
    Return {
        location: SrcSpan,
        type_: Arc<Type>,
        value: Box<Self>,
    },
}
```

#### 2. 类型检查（同方案一）

#### 3. 目标特定代码生成

**JavaScript 生成器**:
```rust
// compiler-core/src/javascript/expression.rs
fn expression(&mut self, expr: &TypedExpr) -> Document<'a> {
    match expr {
        TypedExpr::Return { value, .. } => {
            docvec!["return ", self.expression(value)]
        }
        // ...
    }
}
```

**Erlang 生成器**:
```rust
// compiler-core/src/erlang.rs
fn function_body(&mut self, body: &[TypedStatement]) -> Document<'a> {
    if contains_return(body) {
        // 使用 CPS 变换
        let transformed = cps_transform(body);
        self.statements(&transformed)
    } else {
        self.statements(body)
    }
}
```

### 优点

1. **最佳性能**: JavaScript 直接映射，Erlang 无运行时开销
2. **实现灵活**: 可以针对不同目标优化
3. **概念简单**: AST 层面保持简单
4. **渐进式实现**: 可以先实现 JavaScript 支持，再完善 Erlang

### 缺点

1. **维护成本**: 两套代码生成逻辑
2. **行为一致性**: 需要确保两个目标行为完全一致
3. **测试复杂**: 需要在两个目标上都进行充分测试

### 评分: 8.5/10

---

## 方案对比总结

| 方案 | 实现复杂度 | 运行时性能 | 代码生成质量 | 与 Gleam 哲学一致性 | 总分 |
|------|-----------|-----------|-------------|-------------------|------|
| 方案一：表达式级 Return | 中 | 中（Erlang 有开销） | 中 | 高 | 7.5 |
| 方案二：语句级 Return | 低 | 中 | 中 | 低 | 6.5 |
| 方案三：CPS 变换 | 高 | 高 | 高 | 高 | 7.0 |
| 方案四：混合方案 | 中高 | 高 | 高 | 高 | **8.5** |

## 推荐方案

**推荐采用方案四（混合方案）**，理由如下：

1. **性能最优**: 两个目标平台都能获得最佳性能
2. **实现可行**: 复杂度可控，可以分阶段实现
3. **用户体验好**: 与 Rust 的 return 行为一致
4. **符合 Gleam 设计**: 作为表达式，符合 Gleam 的表达式导向设计

## 实现路线图

### 阶段一：基础设施（1-2 周）
1. 添加 `Token::Return`
2. 添加 `UntypedExpr::Return` 和 `TypedExpr::Return`
3. 实现 Parser 支持
4. 实现基本类型检查

### 阶段二：JavaScript 支持（1 周）
1. 实现 JavaScript 代码生成
2. 添加测试用例
3. 更新文档

### 阶段三：Erlang 支持（2-3 周）
1. 实现 CPS 变换模块
2. 集成到 Erlang 代码生成
3. 处理边界情况
4. 性能测试和优化

### 阶段四：工具链支持（1 周）
1. 格式化器支持
2. 语言服务器支持
3. 错误消息优化

---

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Return 类型一致性
*For any* function with return expressions, the type of every return expression's value SHALL be unifiable with the function's return type.
**Validates: Requirements 3.1, 3.2, 3.3, 3.4**

### Property 2: Return 语义等价性
*For any* function containing return expressions, the compiled output for both Erlang and JavaScript targets SHALL produce semantically equivalent behavior.
**Validates: Requirements 5.3, 6.3**

### Property 3: 控制流正确性
*For any* return expression in a function, all code following that return expression in the same execution path SHALL be unreachable.
**Validates: Requirements 4.1, 4.2**

### Property 4: 嵌套上下文正确性
*For any* return expression inside a nested context (case, block, anonymous function), the return SHALL exit to the correct enclosing function scope.
**Validates: Requirements 7.1, 7.2, 7.3, 7.4**

### Property 5: Round-trip 格式化
*For any* valid Gleam code containing return expressions, formatting then parsing SHALL produce an equivalent AST.
**Validates: Requirements 8.1, 8.2, 8.3**

---

## Error Handling

### 编译时错误

1. **缺少返回表达式错误**
   - 触发条件: `return` 后没有表达式
   - 错误消息: "Expected an expression after `return`. If you want to return nothing, use `return Nil`"
   - 示例:
     ```gleam
     fn example() {
       return  // 错误！
     }
     ```

2. **类型不匹配错误**
   - 触发条件: return 表达式类型与函数返回类型不兼容
   - 错误消息: "Type mismatch: expected `{expected}` but found `{actual}` in return expression"

3. **上下文错误**
   - 触发条件: return 出现在函数体外
   - 错误消息: "return can only be used inside a function body"

4. **不可达代码警告**
   - 触发条件: return 后有代码
   - 警告消息: "Unreachable code after return statement"

### 运行时行为

Return 表达式不会产生运行时错误，所有错误都在编译时捕获。

---

## Testing Strategy

### 单元测试

1. **Lexer 测试**: 验证 `return` token 正确识别
2. **Parser 测试**: 验证各种 return 语法正确解析
3. **类型检查测试**: 验证类型推断和错误检测
4. **代码生成测试**: 验证 Erlang 和 JavaScript 输出

### 属性测试

1. **类型一致性属性测试**: 生成随机函数，验证 return 类型检查
2. **语义等价性属性测试**: 比较两个目标的执行结果
3. **格式化 round-trip 测试**: 验证格式化不改变语义

### 集成测试

1. **端到端测试**: 完整编译和运行包含 return 的程序
2. **跨目标测试**: 确保 Erlang 和 JavaScript 行为一致
3. **边界情况测试**: 嵌套 return、多重 return 等

### 测试框架

- 使用 Rust 的 `#[test]` 进行单元测试
- 使用 `cargo-insta` 进行快照测试
- 使用 `test/language` 目录进行集成测试
