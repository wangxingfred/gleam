# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Gleam is a friendly language for building type-safe systems that scale. This repository contains the Gleam compiler, language server, and related tools written in Rust.

**Current Development Focus**: Implementing a `return` expression feature (using `$return` keyword) to allow early returns from functions. See `.specs/gleam-return-syntax/` for detailed design documents, requirements, and implementation tasks.

## Common Commands

### Building and Testing

```sh
# Build the compiler in release mode
cargo build --release

# Install the Gleam compiler to PATH
cd gleam-bin && cargo install --path . --force --locked

# Run all tests (requires Rust, Erlang, Elixir, NodeJS, Deno, Bun)
make test

# Run only compiler unit tests
cargo test --quiet

# Run clippy linter
cargo clippy

# Run tests in watch mode (requires watchexec)
make test-watch

# Run language integration tests
make language-test
cd test/language && make

# Run JavaScript prelude tests
make javascript-prelude-test
```

### Snapshot Testing

The compiler uses `cargo-insta` for snapshot testing extensively:

```sh
# Run tests to generate new snapshots
cargo test

# Review and accept/reject snapshot changes interactively
cargo insta review

# Accept all new snapshots
cargo insta accept
```

### Running Specific Tests

```sh
# Run tests for a specific package
cargo test -p gleam-core

# Run a specific test by name
cargo test test_name

# Run tests in a specific file
cargo test --test integration_test
```

### Development Tools

```sh
# Set verbose logging for debugging
GLEAM_LOG=trace cargo run

# Check for Rust version issues if clippy fails
rustup upgrade stable

# Print Makefile variables for debugging
make print-VAR_NAME
```

## Architecture

### Project Structure

The repository is a Rust workspace with multiple crates:

- **`compiler-core`**: Pure compilation logic (parsing, analysis, type checking, code generation) with no I/O
- **`compiler-cli`**: Command-line interface wrapping compiler-core with file I/O
- **`language-server`**: LSP implementation for IDE features (autocomplete, hover, code actions)
- **`compiler-wasm`**: WebAssembly interface for browser usage
- **`gleam-bin`**: The main binary crate
- **`test-*`**: Various test helper crates

### Compilation Pipeline

```
Gleam Source → Lexer → Parser → Untyped AST → Type Checker → Typed AST → Code Generator → Erlang/JavaScript
                                        ↑
                          Metadata Deserializer (.cache binaries)
```

Key stages:
1. **Lexer** (`compiler-core/src/parse/lexer.rs`): Tokenization
2. **Parser** (`compiler-core/src/parse.rs`): Syntax analysis → Untyped AST
3. **Type Checker** (`compiler-core/src/type_/expression.rs`): Type inference and validation → Typed AST
4. **Code Generators**:
   - Erlang: `compiler-core/src/erlang.rs`
   - JavaScript: `compiler-core/src/javascript/`

### AST Structure

- **Untyped AST**: `compiler-core/src/ast/untyped.rs` - Output of parser
- **Typed AST**: `compiler-core/src/ast/typed.rs` - Output of type checker
- **Common AST**: `compiler-core/src/ast.rs` - Shared structures

Gleam is expression-oriented. Function bodies contain `Statement`s:
- `Expression`: Bare expressions
- `Assignment`: `let` bindings
- `Use`: Use expressions
- `Assert`: Assertions

### Code Generation

Both targets use a pretty-printing algebra approach:
- Erlang: Single file `erlang.rs` with pattern matching
- JavaScript: Modular structure in `javascript/` directory

**Important**: For the `return` feature implementation, JavaScript directly maps to native `return` statements, while Erlang uses CPS (Continuation-Passing Style) transformation to avoid `throw/catch` (which breaks tail recursion).

### Transform Pipeline

The `compiler-core/src/transform/` directory contains AST transformation passes:
- **CPS transformation** (`transform/cps.rs`): Converts `return` expressions into continuation-passing style for Erlang code generation

## Return Expression Implementation Notes

When working on the return expression feature (`$return` keyword):

1. **Mandatory Expression**: `return` MUST be followed by an expression (no bare `return`). Use `return Nil` for void returns.

2. **CPS Transformation for Erlang**:
   - MUST NOT use `throw/catch` (breaks tail call optimization)
   - Use CPS transformation in `compiler-core/src/transform/cps.rs`
   - Apply transformation during Erlang code generation if `contains_return()` is true
   - Side effects must be preserved correctly

3. **Type Checking**:
   - Return type must unify with function's return type
   - Mark as `previous_panics = true` for unreachable code analysis
   - Similar to `panic` and `todo` expressions

4. **Testing**:
   - Add snapshot tests in `compiler-core/src/erlang/tests/return_expr.rs`
   - Add snapshot tests in `compiler-core/src/javascript/tests/return_expr.rs`
   - Ensure cross-target semantic equivalence
   - Run `cargo insta review` to accept snapshots

5. **Key Files**:
   - Token: `compiler-core/src/parse/token.rs`
   - Lexer: `compiler-core/src/parse/lexer.rs`
   - Parser: `compiler-core/src/parse.rs`
   - AST: `compiler-core/src/ast/untyped.rs`, `compiler-core/src/ast/typed.rs`
   - Type checker: `compiler-core/src/type_/expression.rs`
   - CPS transform: `compiler-core/src/transform/cps.rs`
   - Erlang codegen: `compiler-core/src/erlang.rs`
   - JavaScript codegen: `compiler-core/src/javascript/expression.rs`

## Coding Guidelines

### Rust Practices

- The codebase uses strict clippy lints (see `compiler-core/src/lib.rs`)
- No `unsafe` code allowed
- No `.unwrap()` or `.expect()` - use proper error handling
- Avoid indexing with `[]` - use `.get()` instead
- Run `cargo clippy` before committing

### Testing Philosophy

- Heavy use of snapshot testing with `insta`
- Property-based testing for correctness properties
- Integration tests in `test/language/` directory
- Both Erlang and JavaScript targets must be tested for any language feature

### Error Messages

- Clear, actionable error messages are essential
- Include location information (SrcSpan)
- Provide suggestions for fixes when possible

## Cap'n Proto Schema

If modifying `compiler-core/schema.capnp`:
1. Install Cap'n Proto
2. Uncomment lines in `compiler-core/build.rs`
3. Run `cd compiler-core && cargo build` to regenerate `compiler-core/generated/schema_capnp.rs`

## Additional Resources

- Full compiler documentation: `docs/compiler/README.md`
- Contributing guide: `CONTRIBUTING.md`
- Return syntax spec: `.specs/gleam-return-syntax/`
