
use crate::{assert_module_error, type_::pretty::tests::assert_module_infer};

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
    assert_module_error!(
        r#"
pub fn main() {
  $return 1
}
"#
    );
}

#[test]
fn unreachable_code_after_return() {
    // This might be a warning rather than an error, or just valid but dead code.
    // We'll check what happens.
    assert_module_infer!(
        r#"
pub fn main() -> Int {
  $return 1
  2
}
"#,
        vec![],
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
