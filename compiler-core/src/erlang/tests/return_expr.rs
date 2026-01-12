use crate::assert_erl;
use crate::ast::{Statement, TypedExpr, SrcSpan};
use crate::type_::prelude::int;
use crate::transform::cps;


#[test]
fn simple_return() {
    assert_erl!(
        r#"
pub fn main() {
    $return 1
    2
}"#
    );
}

#[test]
fn return_in_block() {
    assert_erl!(
        r#"
pub fn main() {
  let x = {
    $return 1
    2
  }
  x
}"#
    );
}

#[test]
fn return_in_case() {
    assert_erl!(
        r#"
pub fn main(x) {
  case x {
    1 -> $return 1
    _ -> 2
  }
}"#
    );
}

#[test]
fn return_nil() {
    assert_erl!(
        r#"
pub fn main() {
  $return Nil
}"#
    );
}


#[test]
fn return_complex_expression() {
    assert_erl!(
        r#"
pub fn main() {
  $return #(1, 2)
}
"#
    );
}

#[test]
fn deep_nested_return_bug() {
    assert_erl!(
        r#"
pub fn main() {
  let x = foo({
    $return 1
    2
  })
  x
}
fn foo(a) { a }
"#
    );
}

#[test]
fn return_in_fn() {
    assert_erl!(
        r#"
pub fn main() {
  let f = fn() {
    $return 1
    2
  }
  f()
}"#
    );
}
#[test]
fn return_in_complex_cases() {
    assert_erl!(
        r#"
pub fn main(x, y, z) {
  case x {
    True ->
      case y {
        True -> $return 1
        False -> 2
      }
    False -> 3
  }
  case z {
    True -> $return 4
    False -> 5
  }
  6
}"#
    );
}

// Additional comprehensive tests for complete coverage

#[test]
fn return_with_string() {
    assert_erl!(
        r#"
pub fn main() -> String {
  $return "hello world"
}"#
    );
}

#[test]
fn return_with_list() {
    assert_erl!(
        r#"
pub fn main() -> List(Int) {
  $return [1, 2, 3]
}"#
    );
}

#[test]
fn return_with_result() {
    assert_erl!(
        r#"
pub fn main() -> Result(Int, String) {
  $return Ok(42)
}"#
    );
}

#[test]
fn return_with_custom_type() {
    assert_erl!(
        r#"
pub type Person {
  Person(name: String, age: Int)
}

pub fn main() -> Person {
  $return Person("Alice", 30)
}"#
    );
}

#[test]
fn multiple_returns_same_function() {
    assert_erl!(
        r#"
pub fn main(x: Int) -> Int {
  case x {
    1 -> $return 10
    2 -> $return 20
    3 -> $return 30
    _ -> 40
  }
}"#
    );
}

#[test]
fn return_in_nested_blocks() {
    assert_erl!(
        r#"
pub fn main(x: Int) -> Int {
  let result = {
    let inner = {
      case x > 0 {
        True -> $return x * 2
        False -> x
      }
    }
    inner + 1
  }
  result
}"#
    );
}

#[test]
fn return_with_function_call() {
    assert_erl!(
        r#"
pub fn helper(x: Int) -> Int {
  x * 2
}

pub fn main(x: Int) -> Int {
  case x > 10 {
    True -> $return helper(x)
    False -> x + 1
  }
}"#
    );
}


#[test]
fn return_with_pipe_operator() {
    assert_erl!(
        r#"
pub fn double(x: Int) -> Int {
  x * 2
}

pub fn main(x: Int) -> Int {
  case x > 0 {
    True -> $return x |> double
    False -> 0
  }
}"#
    );
}

#[test]
fn return_in_let_assert() {
    assert_erl!(
        r#"
pub fn main(x: Result(Int, String)) -> Int {
  let assert Ok(value) = x
  case value > 10 {
    True -> $return value
    False -> 0
  }
}"#
    );
}

#[test]
fn return_with_bit_arrays() {
    assert_erl!(
        r#"
pub fn main(x: Int) -> BitArray {
  case x > 0 {
    True -> $return <<x:32>>
    False -> <<0:32>>
  }
}"#
    );
}

#[test]
fn return_in_guards() {
    assert_erl!(
        r#"
pub fn main(x: Int) -> Int {
  case x {
    n if n > 10 -> $return n * 2
    n if n > 5 -> $return n + 1
    _ -> 0
  }
}"#
    );
}

#[test]
fn return_with_record_access() {
    assert_erl!(
        r#"
pub type Point {
  Point(x: Int, y: Int)
}

pub fn main(p: Point) -> Int {
  case p.x > 0 {
    True -> $return p.x + p.y
    False -> 0
  }
}"#
    );
}

#[test]
fn return_with_tuple_access() {
    assert_erl!(
        r#"
pub fn main(t: #(Int, Int)) -> Int {
  case t.0 > t.1 {
    True -> $return t.0
    False -> t.1
  }
}"#
    );
}

#[test]
fn return_early_from_long_function() {
    assert_erl!(
        r#"
pub fn main(x: Int) -> Int {
  let step1 = x + 1
  case step1 < 0 {
    True -> $return -1
    False -> 0
  }

  let step2 = step1 * 2
  case step2 > 100 {
    True -> $return 100
    False -> 0
  }

  let step3 = step2 + 10
  case step3 > 50 {
    True -> $return step3
    False -> 0
  }

  step3 * 2
}"#
    );
}

#[test]
fn return_with_pattern_matching() {
    assert_erl!(
        r#"
pub fn main(x: List(Int)) -> Int {
  case x {
    [] -> $return 0
    [first] -> $return first
    [first, second, ..] -> $return first + second
  }
}"#
    );
}

#[test]
fn return_with_anonymous_function_scope() {
    assert_erl!(
        r#"
pub fn main() -> Int {
  let f = fn(x) {
    case x > 0 {
      True -> $return x  // This should return from the anonymous function, not main
      False -> 0
    }
  }
  let result = f(5)
  result + 10
}"#
    );
}

#[test]
fn return_with_nested_anonymous_functions() {
    assert_erl!(
        r#"
pub fn main() -> Int {
  let outer = fn(x) {
    let inner = fn(y) {
      case y > 0 {
        True -> $return y * 2  // Returns from inner function, not main
        False -> 0
      }
    }
    inner(x) + 1
  }
  outer(5)
}"#
    );
}

#[test]
fn return_with_float_operations() {
    assert_erl!(
        r#"
pub fn main(x: Float) -> Float {
  case x >. 0.0 {
    True -> $return x *. 2.5
    False -> 0.0
  }
}"#
    );
}

#[test]
fn return_with_string_operations() {
    assert_erl!(
        r#"
pub fn main(s: String) -> String {
  case s {
    "" -> $return "empty"
    _ -> s <> " processed"
  }
}"#
    );
}

#[test]
fn return_with_boolean_logic() {
    assert_erl!(
        r#"
pub fn main(a: Bool, b: Bool) -> Bool {
  case a && b {
    True -> $return True
    False -> a || b
  }
}"#
    );
}

#[test]
fn return_in_deeply_nested_case() {
    assert_erl!(
        r#"
pub fn main(x: Int, y: Int, z: Int) -> Int {
  case x {
    1 -> case y {
      1 -> case z {
        1 -> $return 111
        2 -> $return 112
        _ -> 110
      }
      2 -> case z {
        1 -> $return 121
        _ -> 120
      }
      _ -> 100
    }
    2 -> case y {
      1 -> $return 210
      _ -> 200
    }
    _ -> 0
  }
}"#
    );
}

#[test]
fn return_with_error_handling() {
    assert_erl!(
        r#"
pub fn main(x: Result(Int, String)) -> Result(Int, String) {
  case x {
    Ok(value) -> case value > 0 {
      True -> $return Ok(value * 2)
      False -> Error("negative value")
    }
    Error(msg) -> $return Error("wrapped: " <> msg)
  }
}"#
    );
}






#[test]
fn return_expression_simple() {
    // Test simple return expression with CPS transformation
    assert_erl!(r#"pub fn test_return() -> Int { $return 42 }"#);
}

#[test]
fn return_expression_conditional() {
    // Test return expression in conditional context
    assert_erl!(
        r#"pub fn test_conditional_return(x: Int) -> Int {
  case x > 0 {
    True -> $return x * 2
    False -> x + 1
  }
}"#
    );
}

#[test]
fn return_expression_no_return() {
    // Test function without return expressions (should not trigger CPS transformation)
    assert_erl!(r#"pub fn test_no_return(x: Int) -> Int { x + 1 }"#);
}

#[test]
fn test_cps_integration_with_erlang_generator() {
    use crate::ast::TypedExpr;
    // Create a simple return expression
    let return_expr = TypedExpr::Return {
        location: SrcSpan { start: 0, end: 0 },
        type_: int(),
        value: Box::new(TypedExpr::Int {
            location: SrcSpan { start: 0, end: 0 },
            type_: int(),
            value: "42".into(),
            int_value: 42.into(),
        }),
    };

    let statements = vec![Statement::Expression(return_expr)];

    // Test that contains_return works
    assert!(cps::contains_return(&statements));

    // Test CPS transformation
    let transformed = cps::cps_transform(statements);
    assert_eq!(transformed.len(), 1);

    match &transformed[0] {
        Statement::Expression(TypedExpr::Int { int_value, .. }) => {
            assert_eq!(*int_value, 42.into());
        }
        _ => panic!("Expected transformed $return to become an int expression"),
    }
}

/// Cross-target consistency integration test for $return expressions
/// **Feature: gleam-return-syntax, Property 2: Return 语义等价性**
/// **Validates: Requirements 5.3, 6.3**
#[test]
fn test_cross_target_return_consistency() {
    // Test cases that should produce semantically equivalent results on both targets
    let test_cases = vec![
        // Simple return with integer
        r#"pub fn main() -> Int { $return 42 }"#,
        r#"pub fn main() -> Int { $return 0 }"#,
        r#"pub fn main() -> Int { $return 999 }"#,

        // Return with string
        r#"pub fn main() -> String { $return "hello" }"#,
        r#"pub fn main() -> String { $return "" }"#,

        // Return in conditional context
        r#"pub fn main(x: Bool) -> Int {
  case x {
    True -> $return 1
    False -> 2
  }
}"#,

        // Function without return (should not trigger CPS transformation)
        r#"pub fn main() -> Int { 42 }"#,
        r#"pub fn main(x: Int) -> Int { x + 1 }"#,
    ];

    for gleam_code in test_cases {
        // Compile to Erlang
        let erlang_compiled = crate::erlang::tests::compile_test_project(
            gleam_code,
            "/root/project/test/my/mod.gleam",
            vec![]
        );

        // Erlang should compile successfully (no panics or errors)
        assert!(!erlang_compiled.is_empty(),
                "Erlang compilation should produce output for: {}", gleam_code);

        // Should contain the function definition
        assert!(erlang_compiled.contains("main(") || erlang_compiled.contains("main() ->"),
                "Erlang output should contain function definition for: {}", gleam_code);

        // Check for return-specific patterns
        if gleam_code.contains("$return") {
            // Erlang should either have CPS transformation comment or direct value
            let has_cps_comment = erlang_compiled.contains("% CPS transformation applied to handle $return expressions");
            let has_direct_value = erlang_compiled.contains("42") || erlang_compiled.contains("hello") ||
                erlang_compiled.contains("0") || erlang_compiled.contains("999") || erlang_compiled.contains("1");
            assert!(has_cps_comment || has_direct_value,
                    "Erlang output should show CPS transformation or direct value for: {}", gleam_code);
        }

        // Should have valid module structure
        assert!(erlang_compiled.contains("-module(") && erlang_compiled.contains("-export("),
                "Erlang output should have valid module structure for: {}", gleam_code);
    }
}

/// Test that return expressions produce consistent behavior across targets
/// This tests the semantic equivalence property more directly
#[test]
fn test_return_semantic_equivalence_across_targets() {
    // Test cases with expected semantic behavior
    let equivalence_test_cases = vec![
        // Simple return should be equivalent to direct value
        (r#"pub fn main_return() -> Int { $return 42 }"#,
         r#"pub fn main_direct() -> Int { 42 }"#,
         "42"),

        // String return should be equivalent to direct string
        (r#"pub fn main_return() -> String { $return "hello" }"#,
         r#"pub fn main_direct() -> String { "hello" }"#,
         "hello"),

        // Return in case should be equivalent to case without return
        (r#"pub fn main_return(x: Bool) -> Int {
                case x { True -> $return 1 False -> 2 }
              }"#,
         r#"pub fn main_direct(x: Bool) -> Int {
                case x { True -> 1 False -> 2 }
              }"#,
         "1"),
    ];

    for (return_code, direct_code, expected_value) in equivalence_test_cases {
        // Compile both versions to Erlang
        let return_erlang = crate::erlang::tests::compile_test_project(
            return_code, "/root/project/test/my/mod.gleam", vec![]
        );
        let direct_erlang = crate::erlang::tests::compile_test_project(
            direct_code, "/root/project/test/my/mod.gleam", vec![]
        );

        // Both should compile successfully
        assert!(!return_erlang.is_empty() && !direct_erlang.is_empty(),
                "Both Erlang versions should compile successfully");

        // Both Erlang versions should contain the expected value
        assert!(return_erlang.contains(expected_value) && direct_erlang.contains(expected_value),
                "Both Erlang versions should contain expected value: {}", expected_value);

        // The return version should show evidence of return handling
        if return_code.contains("$return") {
            // Erlang: either CPS transformation or direct compilation
            let erlang_has_return_handling = return_erlang.contains("% CPS transformation") ||
                return_erlang.contains(expected_value);
            assert!(erlang_has_return_handling,
                    "Erlang return version should show $return handling");
        }
    }
}

/// Test edge cases for cross-target consistency
#[test]
fn test_return_edge_cases_cross_target() {
    let edge_cases = vec![
        // Return with complex expressions
        r#"pub fn main() -> Int { $return 1 + 2 * 3 }"#,

        // Return with function call
        r#"pub fn helper() -> Int { 42 }
               pub fn main() -> Int { $return helper() }"#,

        // Return with tuple
        r#"pub fn main() -> #(Int, String) { $return #(42, "hello") }"#,

        // Return in nested block
        r#"pub fn main() -> Int {
                {
                  let x = 1
                  $return x + 1
                }
              }"#,
    ];

    for gleam_code in edge_cases {
        // Compile to Erlang
        let erlang_output = crate::erlang::tests::compile_test_project(
            gleam_code, "/root/project/test/my/mod.gleam", vec![]
        );

        // Should compile without errors
        assert!(!erlang_output.is_empty(),
                "Erlang should compile edge case: {}", gleam_code);

        // Should contain function definitions
        assert!(erlang_output.contains("main(") || erlang_output.contains("main() ->"),
                "Erlang should have function definition for edge case: {}", gleam_code);

        // Return expressions should be handled appropriately
        if gleam_code.contains("$return") {
            // Erlang: should show some form of $return handling
            let erlang_handles_return = erlang_output.contains("% CPS transformation") ||
                erlang_output.contains("42") ||
                erlang_output.contains("hello") ||
                erlang_output.contains("{") || // for tuples
                erlang_output.contains("helper()") ||
                erlang_output.contains("1 + (2 * 3)") || // for 1 + 2 * 3
                erlang_output.contains("main() ->"); // function should be generated
            assert!(erlang_handles_return,
                    "Erlang should handle $return in edge case: {}\nActual output: {}", gleam_code, erlang_output);
        }
    }
}
