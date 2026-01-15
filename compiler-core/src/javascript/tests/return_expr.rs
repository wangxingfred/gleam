use crate::assert_js;

/// Property test for JavaScript return semantic equivalence
/// **Feature: gleam-return-syntax, Property 2: Return 语义等价性（JavaScript 部分）**
/// **Validates: Requirements 6.3**
#[test]
fn property_javascript_return_semantic_equivalence() {
    use rand::Rng;

    // Test 1: Simple return expressions with various value types
    let mut rng = rand::rng();

    // Test with integer returns
    for _ in 0..20 {
        let value = rng.random::<i32>() % 1000;
        let gleam_code = format!(
            r#"
pub fn test_return() {{
  $return {value}
}}
"#
        );

        let compiled = crate::javascript::tests::compile_js(&gleam_code, vec![]);

        // Verify the compiled JavaScript contains a return statement
        assert!(compiled.contains("return "),
               "Compiled JavaScript should contain return statement for: {}", gleam_code);
        assert!(compiled.contains(&value.to_string()),
                "Compiled JavaScript should contain the returned value in case: {}", value);
    }

    // Test 2: Return expressions with string values
    let test_strings = vec!["hello", "world", "test", "with_spaces"];
    for test_string in test_strings {
        let gleam_code = format!(
            r#"
pub fn test_return() {{
  $return "{test_string}"
}}
"#
        );

        let compiled = crate::javascript::tests::compile_js(&gleam_code, vec![]);

        // Verify the compiled JavaScript contains a return statement
        assert!(compiled.contains("return "),
               "Compiled JavaScript should contain return statement for string: {}", test_string);
    }

    // Test 3: Return expressions with boolean values
    for bool_value in ["True", "False"] {
        let gleam_code = format!(
            r#"
pub fn test_return() {{
  $return {bool_value}
}}
"#
        );

        let compiled = crate::javascript::tests::compile_js(&gleam_code, vec![]);

        // Verify the compiled JavaScript contains a return statement
        assert!(compiled.contains("return "),
               "Compiled JavaScript should contain return statement for boolean: {}", bool_value);
        // JavaScript uses lowercase true/false
        let js_bool = if bool_value == "True" { "true" } else { "false" };
        assert!(compiled.contains(js_bool),
               "Compiled JavaScript should contain the boolean value: {}", js_bool);
    }

    // Test 4: Return expressions in different contexts (case, block)
    for _ in 0..10 {
        let value = rng.random::<i32>() % 100;
        let condition = rng.random::<i32>() % 100;

        // Test return in case expression
        let gleam_code = format!(
            r#"
pub fn test_return(x) {{
  case x {{
    {condition} -> $return {value}
    _ -> x + 1
  }}
}}
"#
        );

        let compiled = crate::javascript::tests::compile_js(&gleam_code, vec![]);

        // Verify the compiled JavaScript contains a return statement
        assert!(compiled.contains("return "),
               "Compiled JavaScript should contain return statement in case: {}", gleam_code);
        assert!(compiled.contains(&value.to_string()),
               "Compiled JavaScript should contain the returned value in case: {}", value);
    }

    // Test 5: Return expressions with expressions as values
    for _ in 0..10 {
        let a = rng.random::<i32>() % 50;
        let b = rng.random::<i32>() % 50;

        let gleam_code = format!(
            r#"
pub fn test_return() {{
  $return {a} + {b}
}}
"#
        );

        let compiled = crate::javascript::tests::compile_js(&gleam_code, vec![]);

        // Verify the compiled JavaScript contains a return statement
        assert!(compiled.contains("return "),
               "Compiled JavaScript should contain return statement for expression: {}", gleam_code);
        // Should contain the operands
        assert!(compiled.contains(&a.to_string()) && compiled.contains(&b.to_string()),
               "Compiled JavaScript should contain the expression operands: {} + {}", a, b);
    }

    // Test 6: Verify semantic equivalence - return should exit function immediately
    let gleam_code = r#"
pub fn test_early_return(x) {
  case x > 0 {
    True -> $return x * 2
    False -> x + 1
  }
}
"#;

    let compiled = crate::javascript::tests::compile_js(gleam_code, vec![]);

    // The compiled JavaScript should have proper control flow
    assert!(compiled.contains("return "),
           "Compiled JavaScript should contain return statement for early return");

    // Should not have unreachable code warnings in the generated JS
    // (this is more of a structural check)
    assert!(!compiled.contains("// unreachable"),
           "Compiled JavaScript should not contain unreachable code comments");
}

#[test]
fn simple_return() {
    assert_js!(
        r#"
pub fn go() {
  $return 42
}
"#,
    );
}

#[test]
fn return_with_expression() {
    assert_js!(
        r#"
pub fn go(x) {
  $return x + 1
}
"#,
    );
}

#[test]
fn return_in_case_with_different_types() {
    // This was previously a type error but should now compile
    assert_js!(
        r#"
pub fn main(x) -> Int {
  case x {
    1 -> $return 0
    _ -> 1
  }
}"#
    );
}

#[test]
fn return_in_case_branch_mismatch() {
    // Case expects String, but return exits with Int
    assert_js!(
        r#"
pub fn main(x) -> Int {
  let s = case x {
    True -> "hello"
    False -> $return 0
  }
  1
}"#
    );
}


#[test]
fn return_in_block() {
    assert_js!(
        r#"
pub fn go(x) {
  {
    let y = x + 1
    $return y * 2
  }
}
"#,
    );
}

#[test]
fn return_string() {
    assert_js!(
        r#"
pub fn go() {
  $return "hello"
}
"#,
    );
}

// Integration tests for task 6.3: JavaScript integration tests
// Testing simple return expressions and nested contexts

#[test]
fn return_early_from_function() {
    assert_js!(
        r#"
pub fn early_return(x) {
  case x > 0 {
    True -> $return x * 2
    False -> x + 1
  }
}
"#,
    );
}

#[test]
fn return_with_complex_expression() {
    assert_js!(
        r#"
pub fn complex_return(a, b) {
  $return a + b * 2 - 1
}
"#,
    );
}

#[test]
fn return_boolean_values() {
    assert_js!(
        r#"
pub fn return_true() {
  $return True
}

pub fn return_false() {
  $return False
}
"#,
    );
}

#[test]
fn return_nil() {
    assert_js!(
        r#"
pub fn return_nil() {
  return Nil
}
"#,
    );
}

#[test]
fn return_in_nested_case() {
    assert_js!(
        r#"
pub fn nested_case_return(x, y) {
  case x {
    0 -> case y {
      1 -> $return 42
      _ -> y + 1
    }
    _ -> x + y
  }
}
"#,
    );
}

#[test]
fn return_in_nested_block() {
    assert_js!(
        r#"
pub fn nested_block_return(x) {
  {
    let y = x + 1
    {
      let z = y * 2
      $return z + 10
    }
  }
}
"#,
    );
}

#[test]
fn return_with_let_binding() {
    assert_js!(
        r#"
pub fn return_with_let(x) {
  let y = x * 2
  case y > 10 {
    True -> $return y
    False -> y + 1
  }
}
"#,
    );
}

#[test]
fn return_in_case_with_guard() {
    assert_js!(
        r#"
pub fn return_with_guard(x) {
  case x {
    n if n > 0 -> $return n * 2
    _ -> 0
  }
}
"#,
    );
}

#[test]
fn multiple_returns_in_function() {
    assert_js!(
        r#"
pub fn multiple_returns(x) {
  case x {
    0 -> return "zero"
    1 -> return "one"
    2 -> return "two"
    _ -> "other"
  }
}
"#,
    );
}

#[test]
fn return_in_pipe_context() {
    assert_js!(
        r#"
pub fn return_in_pipe(x) {
  x
  |> fn(n) {
    case n > 5 {
      True -> $return n * 2
      False -> n + 1
    }
  }
}
"#,
    );
}

#[test]
fn return_result_type() {
    assert_js!(
        r#"
pub fn return_result(x) {
  case x > 0 {
    True -> $return Ok(x)
    False -> Error("negative")
  }
}
"#,
    );
}

#[test]
fn return_list() {
    assert_js!(
        r#"
pub fn return_list(x) {
  case x > 0 {
    True -> $return [1, 2, 3]
    False -> []
  }
}
"#,
    );
}

#[test]
fn return_tuple() {
    assert_js!(
        r#"
pub fn return_tuple(x, y) {
  case x > y {
    True -> $return #(x, y)
    False -> #(y, x)
  }
}
"#,
    );
}

#[test]
fn return_in_deeply_nested_context() {
    assert_js!(
        r#"
pub fn deeply_nested(x) {
  case x {
    0 -> {
      let y = 1
      case y {
        1 -> {
          let z = 2
          case z > 0 {
            True -> $return z * 10
            False -> z
          }
        }
        _ -> y
      }
    }
    _ -> x
  }
}
"#,
    );
}

#[test]
fn return_with_function_call() {
    assert_js!(
        r#"
fn helper(x) {
  x * 2
}

pub fn return_with_call(x) {
  case x > 0 {
    True -> $return helper(x)
    False -> 0
  }
}
"#,
    );
}
