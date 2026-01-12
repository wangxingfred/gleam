
use crate::assert_format;

#[test]
fn return_simple() {
    assert_format!(
        r#"pub fn main() {
  $return 1
}
"#
    );
}

#[test]
fn return_in_block() {
    assert_format!(
        r#"pub fn main() {
  { $return 1 }
}
"#
    );
}

#[test]
fn return_nested_block() {
    assert_format!(
        r#"pub fn main() {
  { { $return 1 } }
}
"#
    );
}

#[test]
fn return_in_let() {
    assert_format!(
        r#"pub fn main() {
  let x = $return 1
  x
}
"#
    );
}

#[test]
fn return_in_let_assert() {
    assert_format!(
        r#"pub fn main() {
  let assert x = $return 1
  x
}
"#
    );
}

#[test]
fn return_in_binop_right() {
    assert_format!(
        r#"pub fn main() {
  1 + $return 2
}
"#
    );
}

#[test]
fn return_in_binop_left() {
    assert_format!(
        r#"pub fn main() {
  $return 1 + 2
}
"#
    );
}

#[test]
fn return_in_pipe() {
    assert_format!(
        r#"pub fn main() {
  1
  |> fn(x) { $return x }
}
"#
    );
}

#[test]
fn return_in_case_branch() {
    assert_format!(
        r#"pub fn main() {
  case 1 {
    _ -> $return 1
  }
}
"#
    );
}

#[test]
fn return_in_case_subject() {
    assert_format!(
        r#"pub fn main() {
  case $return 1 {
    _ -> 1
  }
}
"#
    );
}

#[test]
fn return_in_list() {
    assert_format!(
        r#"pub fn main() {
  [$return 1, 2]
}
"#
    );
}

#[test]
fn return_in_tuple() {
    assert_format!(
        r#"pub fn main() {
  #($return 1, 2)
}
"#
    );
}

#[test]
fn return_in_call_arg() {
    assert_format!(
        r#"pub fn main() {
  f($return 1)
}
"#
    );
}

#[test]
fn return_in_bit_array() {
    assert_format!(
        r#"pub fn main() {
  <<$return 1>>
}
"#
    );
}

#[test]
fn return_in_record_update() {
    assert_format!(
        r#"pub fn main() {
  Model(..model, field: $return 1)
}
"#
    );
}

#[test]
fn return_negated_bool() {
    assert_format!(
        r#"pub fn main() {
  !$return True
}
"#
    );
}

#[test]
fn return_negated_int() {
    assert_format!(
        r#"pub fn main() {
  -$return 1
}
"#
    );
}

#[test]
fn return_long_expression() {
    assert_format!(
        r#"pub fn main() {
  $return really_long_function_call(
    arg1,
    arg2,
    arg3,
    arg4,
    arg5,
    arg6,
    arg7,
    arg8,
  )
}
"#
    );
}

#[test]
fn return_with_comment() {
    assert_format!(
        r#"pub fn main() {
  // returning
  $return 1
}
"#
    );
}
