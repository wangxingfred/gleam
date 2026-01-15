
use crate::{assert_module_error, assert_module_infer};

#[test]
fn return_type_mismatch() {
    assert_module_error!(
        r#"
pub fn main() -> Int {
  $return "not an int"
}
"#
    );
}

#[test]
fn return_type_mismatch_in_block() {
    assert_module_error!(
        r#"
pub fn main() -> Int {
  let x = {
    $return "not an int"
  }
  1
}
"#
    );
}

#[test]
fn return_in_void_function() {
    // Functions without explicit return type return Nil
    // So this should work fine
    assert_module_infer!(
        r#"
pub fn main() {
  $return Nil
}
"#,
        vec![("main", "fn() -> Nil")]
    );
}

#[test]
fn unreachable_code_after_return() {
    // Code after return is unreachable and should trigger a warning
    // The function itself should still type check
    assert_module_infer!(
        r#"
pub fn main() -> Int {
  $return 1
  2
}
"#,
        vec![("main", "fn() -> Int")],
    );
}

#[test]
fn return_in_case() {
    assert_module_error!(
        r#"
pub fn main() -> Int {
  case True {
    True -> $return "not an int"
    False -> 1
  }
}
"#
    );
}

#[test]
fn correct_return_type() {
    assert_module_infer!(
        r#"
pub fn main() -> Int {
  $return 1
}
"#,
        vec![("main", "fn() -> Int")]
    );
}

#[test]
fn return_in_case_early_exit_with_different_branch_types() {
    // This is the key test case from the user's bug report
    // The case expression expects Nucleotide type, but one branch has $return
    // which should work like panic/todo and unify with any expected type
    assert_module_infer!(
        r#"
pub type Nucleotide {
  Adenine
  Cytosine
}

pub fn decode(n: Int) -> Result(Nucleotide, Nil) {
  let nucleotide = case n {
    0 -> Adenine
    1 -> $return Error(Nil)
    _ -> Cytosine
  }
  Ok(nucleotide)
}
"#,
        vec![
            ("Adenine", "Nucleotide"),
            ("Cytosine", "Nucleotide"),
            ("decode", "fn(Int) -> Result(Nucleotide, Nil)")
        ]
    );
}

#[test]
fn return_in_case_multiple_branches() {
    // Multiple branches can use $return with different types
    assert_module_infer!(
        r#"
pub fn example(n: Int) -> Result(String, Nil) {
  let x = case n {
    0 -> "zero"
    1 -> $return Error(Nil)
    2 -> "two"
    _ -> $return Ok("early")
  }
  Ok(x)
}
"#,
        vec![("example", "fn(Int) -> Result(String, Nil)")]
    );
}

#[test]
fn return_in_nested_case() {
    assert_module_infer!(
        r#"
pub fn nested(a: Bool, b: Bool) -> Result(Int, Nil) {
  let x = case a {
    True -> case b {
      True -> 1
      False -> $return Error(Nil)
    }
    False -> 2
  }
  Ok(x)
}
"#,
        vec![("nested", "fn(Bool, Bool) -> Result(Int, Nil)")]
    );
}

#[test]
fn return_in_let_binding() {
    // $return in let binding should work like panic/todo
    assert_module_infer!(
        r#"
pub fn example(n: Int) -> String {
  let x = case n > 0 {
    True -> n
    False -> $return "negative"
  }
  int_to_string(x)
}

pub fn int_to_string(n: Int) -> String {
  "todo"
}
"#,
        vec![
            ("example", "fn(Int) -> String"),
            ("int_to_string", "fn(Int) -> String")
        ]
    );
}

#[test]
fn return_in_if_else_branch() {
    assert_module_infer!(
        r#"
pub fn example(n: Int) -> Result(Int, String) {
  let x = case n {
    0 -> $return Error("zero")
    _ -> n
  }
  Ok(x * 2)
}
"#,
        vec![("example", "fn(Int) -> Result(Int, String)")]
    );
}

#[test]
fn return_with_complex_result_type() {
    // Based on the user's actual code
    assert_module_infer!(
        r#"
pub type Nucleotide {
  Adenine
  Cytosine
  Guanine
  Thymine
}

pub fn decode_nucleotide(n: Int) -> Result(Nucleotide, Nil) {
  case n {
    0 -> Ok(Adenine)
    1 -> Ok(Cytosine)
    2 -> Ok(Guanine)
    3 -> Ok(Thymine)
    _ -> Error(Nil)
  }
}

pub fn decode(dna: List(Int)) -> Result(List(Nucleotide), Nil) {
  case dna {
    [first, ..rest] -> {
      let nucleotide = case decode_nucleotide(first) {
        Ok(n) -> n
        Error(_) -> $return Error(Nil)
      }
      case decode(rest) {
        Ok(rest_decoded) -> Ok([nucleotide, ..rest_decoded])
        error -> error
      }
    }
    [] -> Ok([])
  }
}
"#,
        vec![
            ("Adenine", "Nucleotide"),
            ("Cytosine", "Nucleotide"),
            ("Guanine", "Nucleotide"),
            ("Thymine", "Nucleotide"),
            ("decode", "fn(List(Int)) -> Result(List(Nucleotide), Nil)"),
            ("decode_nucleotide", "fn(Int) -> Result(Nucleotide, Nil)")
        ]
    );
}

#[test]
fn return_in_case_guard() {
    assert_module_infer!(
        r#"
pub fn example(n: Int) -> Result(Int, Nil) {
  let x = case n {
    n if n < 0 -> $return Error(Nil)
    n -> n
  }
  Ok(x * 2)
}
"#,
        vec![("example", "fn(Int) -> Result(Int, Nil)")]
    );
}

#[test]
fn return_unifies_with_different_expected_types() {
    // $return should unify with any expected type in its context
    assert_module_infer!(
        r#"
pub fn example(flag: Bool) -> String {
  // Here case expects Int but one branch has $return
  let n: Int = case flag {
    True -> 42
    False -> $return "early exit"
  }
  "unreachable"
}
"#,
        vec![("example", "fn(Bool) -> String")]
    );
}

#[test]
fn return_value_must_match_function_return_type() {
    // The value given to $return must still unify with the function's return type
    assert_module_error!(
        r#"
pub fn example() -> Int {
  let x = case True {
    True -> 1
    False -> $return "wrong type"
  }
  x
}
"#
    );
}
