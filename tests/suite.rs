//! Conformance tests from [nst/JSONTestSuite](https://github.com/nst/JSONTestSuite).
//!
//! Fixtures live in `tests/json/suite/` (`test_parsing/` from upstream).
//! - `y_*` accept · `n_*` reject · `i_*` implementation-defined (no panic)
//!
//! One `#[test]` per fixture (~315). Skipped deep-nesting stress files:
//! `i_structure_500_nested_arrays`, `n_structure_100000_opening_arrays`,
//! `n_structure_open_array_object`.

use rs_jq::json::json_load_slice;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/json/suite")
        .join(name)
}

fn parse_named(name: &str) -> Result<(), String> {
    let bytes = std::fs::read(fixture(name)).map_err(|e| e.to_string())?;
    json_load_slice(&bytes)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn assert_accept(name: &str) {
    if let Err(err) = parse_named(name) {
        panic!("{name}: expected accept, got: {err}");
    }
}

fn assert_reject(name: &str) {
    if parse_named(name).is_ok() {
        panic!("{name}: expected reject, but parse succeeded");
    }
}

fn assert_no_panic(name: &str) {
    let bytes = std::fs::read(fixture(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = json_load_slice(&bytes);
    }));
    assert!(
        result.is_ok(),
        "{name}: panicked on implementation-defined input"
    );
}

// --- must accept (y_) ---
#[test]
fn y_array_arraysWithSpaces() {
    assert_accept("y_array_arraysWithSpaces.json");
}

#[test]
fn y_array_empty_minus_string() {
    assert_accept("y_array_empty-string.json");
}

#[test]
fn y_array_empty() {
    assert_accept("y_array_empty.json");
}

#[test]
fn y_array_ending_with_newline() {
    assert_accept("y_array_ending_with_newline.json");
}

#[test]
fn y_array_false() {
    assert_accept("y_array_false.json");
}

#[test]
fn y_array_heterogeneous() {
    assert_accept("y_array_heterogeneous.json");
}

#[test]
fn y_array_null() {
    assert_accept("y_array_null.json");
}

#[test]
fn y_array_with_1_and_newline() {
    assert_accept("y_array_with_1_and_newline.json");
}

#[test]
fn y_array_with_leading_space() {
    assert_accept("y_array_with_leading_space.json");
}

#[test]
fn y_array_with_several_null() {
    assert_accept("y_array_with_several_null.json");
}

#[test]
fn y_array_with_trailing_space() {
    assert_accept("y_array_with_trailing_space.json");
}

#[test]
fn y_number() {
    assert_accept("y_number.json");
}

#[test]
fn y_number_0e_plus_1() {
    assert_accept("y_number_0e+1.json");
}

#[test]
fn y_number_0e1() {
    assert_accept("y_number_0e1.json");
}

#[test]
fn y_number_after_space() {
    assert_accept("y_number_after_space.json");
}

#[test]
fn y_number_double_close_to_zero() {
    assert_accept("y_number_double_close_to_zero.json");
}

#[test]
fn y_number_int_with_exp() {
    assert_accept("y_number_int_with_exp.json");
}

#[test]
fn y_number_minus_zero() {
    assert_accept("y_number_minus_zero.json");
}

#[test]
fn y_number_negative_int() {
    assert_accept("y_number_negative_int.json");
}

#[test]
fn y_number_negative_one() {
    assert_accept("y_number_negative_one.json");
}

#[test]
fn y_number_negative_zero() {
    assert_accept("y_number_negative_zero.json");
}

#[test]
fn y_number_real_capital_e() {
    assert_accept("y_number_real_capital_e.json");
}

#[test]
fn y_number_real_capital_e_neg_exp() {
    assert_accept("y_number_real_capital_e_neg_exp.json");
}

#[test]
fn y_number_real_capital_e_pos_exp() {
    assert_accept("y_number_real_capital_e_pos_exp.json");
}

#[test]
fn y_number_real_exponent() {
    assert_accept("y_number_real_exponent.json");
}

#[test]
fn y_number_real_fraction_exponent() {
    assert_accept("y_number_real_fraction_exponent.json");
}

#[test]
fn y_number_real_neg_exp() {
    assert_accept("y_number_real_neg_exp.json");
}

#[test]
fn y_number_real_pos_exponent() {
    assert_accept("y_number_real_pos_exponent.json");
}

#[test]
fn y_number_simple_int() {
    assert_accept("y_number_simple_int.json");
}

#[test]
fn y_number_simple_real() {
    assert_accept("y_number_simple_real.json");
}

#[test]
fn y_object() {
    assert_accept("y_object.json");
}

#[test]
fn y_object_basic() {
    assert_accept("y_object_basic.json");
}

#[test]
fn y_object_duplicated_key() {
    assert_accept("y_object_duplicated_key.json");
}

#[test]
fn y_object_duplicated_key_and_value() {
    assert_accept("y_object_duplicated_key_and_value.json");
}

#[test]
fn y_object_empty() {
    assert_accept("y_object_empty.json");
}

#[test]
fn y_object_empty_key() {
    assert_accept("y_object_empty_key.json");
}

#[test]
fn y_object_escaped_null_in_key() {
    assert_accept("y_object_escaped_null_in_key.json");
}

#[test]
fn y_object_extreme_numbers() {
    assert_accept("y_object_extreme_numbers.json");
}

#[test]
fn y_object_long_strings() {
    assert_accept("y_object_long_strings.json");
}

#[test]
fn y_object_simple() {
    assert_accept("y_object_simple.json");
}

#[test]
fn y_object_string_unicode() {
    assert_accept("y_object_string_unicode.json");
}

#[test]
fn y_object_with_newlines() {
    assert_accept("y_object_with_newlines.json");
}

#[test]
fn y_string_1_2_3_bytes_UTF_minus_8_sequences() {
    assert_accept("y_string_1_2_3_bytes_UTF-8_sequences.json");
}

#[test]
fn y_string_accepted_surrogate_pair() {
    assert_accept("y_string_accepted_surrogate_pair.json");
}

#[test]
fn y_string_accepted_surrogate_pairs() {
    assert_accept("y_string_accepted_surrogate_pairs.json");
}

#[test]
fn y_string_allowed_escapes() {
    assert_accept("y_string_allowed_escapes.json");
}

#[test]
fn y_string_backslash_and_u_escaped_zero() {
    assert_accept("y_string_backslash_and_u_escaped_zero.json");
}

#[test]
fn y_string_backslash_doublequotes() {
    assert_accept("y_string_backslash_doublequotes.json");
}

#[test]
fn y_string_comments() {
    assert_accept("y_string_comments.json");
}

#[test]
fn y_string_double_escape_a() {
    assert_accept("y_string_double_escape_a.json");
}

#[test]
fn y_string_double_escape_n() {
    assert_accept("y_string_double_escape_n.json");
}

#[test]
fn y_string_escaped_control_character() {
    assert_accept("y_string_escaped_control_character.json");
}

#[test]
fn y_string_escaped_noncharacter() {
    assert_accept("y_string_escaped_noncharacter.json");
}

#[test]
fn y_string_in_array() {
    assert_accept("y_string_in_array.json");
}

#[test]
fn y_string_in_array_with_leading_space() {
    assert_accept("y_string_in_array_with_leading_space.json");
}

#[test]
fn y_string_last_surrogates_1_and_2() {
    assert_accept("y_string_last_surrogates_1_and_2.json");
}

#[test]
fn y_string_nbsp_uescaped() {
    assert_accept("y_string_nbsp_uescaped.json");
}

#[test]
fn y_string_nonCharacterInUTF_minus_8_U_plus_10FFFF() {
    assert_accept("y_string_nonCharacterInUTF-8_U+10FFFF.json");
}

#[test]
fn y_string_nonCharacterInUTF_minus_8_U_plus_FFFF() {
    assert_accept("y_string_nonCharacterInUTF-8_U+FFFF.json");
}

#[test]
fn y_string_null_escape() {
    assert_accept("y_string_null_escape.json");
}

#[test]
fn y_string_one_minus_byte_minus_utf_minus_8() {
    assert_accept("y_string_one-byte-utf-8.json");
}

#[test]
fn y_string_pi() {
    assert_accept("y_string_pi.json");
}

#[test]
fn y_string_reservedCharacterInUTF_minus_8_U_plus_1BFFF() {
    assert_accept("y_string_reservedCharacterInUTF-8_U+1BFFF.json");
}

#[test]
fn y_string_simple_ascii() {
    assert_accept("y_string_simple_ascii.json");
}

#[test]
fn y_string_space() {
    assert_accept("y_string_space.json");
}

#[test]
fn y_string_surrogates_U_plus_1D11E_MUSICAL_SYMBOL_G_CLEF() {
    assert_accept("y_string_surrogates_U+1D11E_MUSICAL_SYMBOL_G_CLEF.json");
}

#[test]
fn y_string_three_minus_byte_minus_utf_minus_8() {
    assert_accept("y_string_three-byte-utf-8.json");
}

#[test]
fn y_string_two_minus_byte_minus_utf_minus_8() {
    assert_accept("y_string_two-byte-utf-8.json");
}

#[test]
fn y_string_u_plus_2028_line_sep() {
    assert_accept("y_string_u+2028_line_sep.json");
}

#[test]
fn y_string_u_plus_2029_par_sep() {
    assert_accept("y_string_u+2029_par_sep.json");
}

#[test]
fn y_string_uEscape() {
    assert_accept("y_string_uEscape.json");
}

#[test]
fn y_string_uescaped_newline() {
    assert_accept("y_string_uescaped_newline.json");
}

#[test]
fn y_string_unescaped_char_delete() {
    assert_accept("y_string_unescaped_char_delete.json");
}

#[test]
fn y_string_unicode() {
    assert_accept("y_string_unicode.json");
}

#[test]
fn y_string_unicodeEscapedBackslash() {
    assert_accept("y_string_unicodeEscapedBackslash.json");
}

#[test]
fn y_string_unicode_2() {
    assert_accept("y_string_unicode_2.json");
}

#[test]
fn y_string_unicode_U_plus_10FFFE_nonchar() {
    assert_accept("y_string_unicode_U+10FFFE_nonchar.json");
}

#[test]
fn y_string_unicode_U_plus_1FFFE_nonchar() {
    assert_accept("y_string_unicode_U+1FFFE_nonchar.json");
}

#[test]
fn y_string_unicode_U_plus_200B_ZERO_WIDTH_SPACE() {
    assert_accept("y_string_unicode_U+200B_ZERO_WIDTH_SPACE.json");
}

#[test]
fn y_string_unicode_U_plus_2064_invisible_plus() {
    assert_accept("y_string_unicode_U+2064_invisible_plus.json");
}

#[test]
fn y_string_unicode_U_plus_FDD0_nonchar() {
    assert_accept("y_string_unicode_U+FDD0_nonchar.json");
}

#[test]
fn y_string_unicode_U_plus_FFFE_nonchar() {
    assert_accept("y_string_unicode_U+FFFE_nonchar.json");
}

#[test]
fn y_string_unicode_escaped_double_quote() {
    assert_accept("y_string_unicode_escaped_double_quote.json");
}

#[test]
fn y_string_utf8() {
    assert_accept("y_string_utf8.json");
}

#[test]
fn y_string_with_del_character() {
    assert_accept("y_string_with_del_character.json");
}

#[test]
fn y_structure_lonely_false() {
    assert_accept("y_structure_lonely_false.json");
}

#[test]
fn y_structure_lonely_int() {
    assert_accept("y_structure_lonely_int.json");
}

#[test]
fn y_structure_lonely_negative_real() {
    assert_accept("y_structure_lonely_negative_real.json");
}

#[test]
fn y_structure_lonely_null() {
    assert_accept("y_structure_lonely_null.json");
}

#[test]
fn y_structure_lonely_string() {
    assert_accept("y_structure_lonely_string.json");
}

#[test]
fn y_structure_lonely_true() {
    assert_accept("y_structure_lonely_true.json");
}

#[test]
fn y_structure_string_empty() {
    assert_accept("y_structure_string_empty.json");
}

#[test]
fn y_structure_trailing_newline() {
    assert_accept("y_structure_trailing_newline.json");
}

#[test]
fn y_structure_true_in_array() {
    assert_accept("y_structure_true_in_array.json");
}

#[test]
fn y_structure_whitespace_array() {
    assert_accept("y_structure_whitespace_array.json");
}

// --- must reject (n_) ---
#[test]
fn n_array_1_true_without_comma() {
    assert_reject("n_array_1_true_without_comma.json");
}

#[test]
fn n_array_a_invalid_utf8() {
    assert_reject("n_array_a_invalid_utf8.json");
}

#[test]
fn n_array_colon_instead_of_comma() {
    assert_reject("n_array_colon_instead_of_comma.json");
}

#[test]
fn n_array_comma_after_close() {
    assert_reject("n_array_comma_after_close.json");
}

#[test]
fn n_array_comma_and_number() {
    assert_reject("n_array_comma_and_number.json");
}

#[test]
fn n_array_double_comma() {
    assert_reject("n_array_double_comma.json");
}

#[test]
fn n_array_double_extra_comma() {
    assert_reject("n_array_double_extra_comma.json");
}

#[test]
fn n_array_extra_close() {
    assert_reject("n_array_extra_close.json");
}

#[test]
fn n_array_extra_comma() {
    assert_reject("n_array_extra_comma.json");
}

#[test]
fn n_array_incomplete() {
    assert_reject("n_array_incomplete.json");
}

#[test]
fn n_array_incomplete_invalid_value() {
    assert_reject("n_array_incomplete_invalid_value.json");
}

#[test]
fn n_array_inner_array_no_comma() {
    assert_reject("n_array_inner_array_no_comma.json");
}

#[test]
fn n_array_invalid_utf8() {
    assert_reject("n_array_invalid_utf8.json");
}

#[test]
fn n_array_items_separated_by_semicolon() {
    assert_reject("n_array_items_separated_by_semicolon.json");
}

#[test]
fn n_array_just_comma() {
    assert_reject("n_array_just_comma.json");
}

#[test]
fn n_array_just_minus() {
    assert_reject("n_array_just_minus.json");
}

#[test]
fn n_array_missing_value() {
    assert_reject("n_array_missing_value.json");
}

#[test]
fn n_array_newlines_unclosed() {
    assert_reject("n_array_newlines_unclosed.json");
}

#[test]
fn n_array_number_and_comma() {
    assert_reject("n_array_number_and_comma.json");
}

#[test]
fn n_array_number_and_several_commas() {
    assert_reject("n_array_number_and_several_commas.json");
}

#[test]
fn n_array_spaces_vertical_tab_formfeed() {
    assert_reject("n_array_spaces_vertical_tab_formfeed.json");
}

#[test]
fn n_array_star_inside() {
    assert_reject("n_array_star_inside.json");
}

#[test]
fn n_array_unclosed() {
    assert_reject("n_array_unclosed.json");
}

#[test]
fn n_array_unclosed_trailing_comma() {
    assert_reject("n_array_unclosed_trailing_comma.json");
}

#[test]
fn n_array_unclosed_with_new_lines() {
    assert_reject("n_array_unclosed_with_new_lines.json");
}

#[test]
fn n_array_unclosed_with_object_inside() {
    assert_reject("n_array_unclosed_with_object_inside.json");
}

#[test]
fn n_incomplete_false() {
    assert_reject("n_incomplete_false.json");
}

#[test]
fn n_incomplete_null() {
    assert_reject("n_incomplete_null.json");
}

#[test]
fn n_incomplete_true() {
    assert_reject("n_incomplete_true.json");
}

#[test]
fn n_multidigit_number_then_00() {
    assert_reject("n_multidigit_number_then_00.json");
}

#[test]
fn n_number_plus_plus() {
    assert_reject("n_number_++.json");
}

#[test]
fn n_number_plus_1() {
    assert_reject("n_number_+1.json");
}

#[test]
fn n_number_plus_Inf() {
    assert_reject("n_number_+Inf.json");
}

#[test]
fn n_number_minus_01() {
    assert_reject("n_number_-01.json");
}

#[test]
fn n_number_minus_1_0() {
    assert_reject("n_number_-1.0..json");
}

#[test]
fn n_number_minus_2() {
    assert_reject("n_number_-2..json");
}

#[test]
fn n_number_minus_NaN() {
    assert_reject("n_number_-NaN.json");
}

#[test]
fn n_number_minus_1() {
    assert_reject("n_number_.-1.json");
}

#[test]
fn n_number_2e_minus_3() {
    assert_reject("n_number_.2e-3.json");
}

#[test]
fn n_number_0_1_2() {
    assert_reject("n_number_0.1.2.json");
}

#[test]
fn n_number_0_3e_plus() {
    assert_reject("n_number_0.3e+.json");
}

#[test]
fn n_number_0_3e() {
    assert_reject("n_number_0.3e.json");
}

#[test]
fn n_number_0_e1() {
    assert_reject("n_number_0.e1.json");
}

#[test]
fn n_number_0_capital_E_plus() {
    assert_reject("n_number_0_capital_E+.json");
}

#[test]
fn n_number_0_capital_E() {
    assert_reject("n_number_0_capital_E.json");
}

#[test]
fn n_number_0e_plus() {
    assert_reject("n_number_0e+.json");
}

#[test]
fn n_number_0e() {
    assert_reject("n_number_0e.json");
}

#[test]
fn n_number_1_0e_plus() {
    assert_reject("n_number_1.0e+.json");
}

#[test]
fn n_number_1_0e_minus() {
    assert_reject("n_number_1.0e-.json");
}

#[test]
fn n_number_1_0e() {
    assert_reject("n_number_1.0e.json");
}

#[test]
fn n_number_1_000() {
    assert_reject("n_number_1_000.json");
}

#[test]
fn n_number_1eE2() {
    assert_reject("n_number_1eE2.json");
}

#[test]
fn n_number_2_e_plus_3() {
    assert_reject("n_number_2.e+3.json");
}

#[test]
fn n_number_2_e_minus_3() {
    assert_reject("n_number_2.e-3.json");
}

#[test]
fn n_number_2_e3() {
    assert_reject("n_number_2.e3.json");
}

#[test]
fn n_number_9_e_plus() {
    assert_reject("n_number_9.e+.json");
}

#[test]
fn n_number_Inf() {
    assert_reject("n_number_Inf.json");
}

#[test]
fn n_number_NaN() {
    assert_reject("n_number_NaN.json");
}

#[test]
fn n_number_U_plus_FF11_fullwidth_digit_one() {
    assert_reject("n_number_U+FF11_fullwidth_digit_one.json");
}

#[test]
fn n_number_expression() {
    assert_reject("n_number_expression.json");
}

#[test]
fn n_number_hex_1_digit() {
    assert_reject("n_number_hex_1_digit.json");
}

#[test]
fn n_number_hex_2_digits() {
    assert_reject("n_number_hex_2_digits.json");
}

#[test]
fn n_number_infinity() {
    assert_reject("n_number_infinity.json");
}

#[test]
fn n_number_invalid_plus_minus() {
    assert_reject("n_number_invalid+-.json");
}

#[test]
fn n_number_invalid_minus_negative_minus_real() {
    assert_reject("n_number_invalid-negative-real.json");
}

#[test]
fn n_number_invalid_minus_utf_minus_8_minus_in_minus_bigger_minus_int() {
    assert_reject("n_number_invalid-utf-8-in-bigger-int.json");
}

#[test]
fn n_number_invalid_minus_utf_minus_8_minus_in_minus_exponent() {
    assert_reject("n_number_invalid-utf-8-in-exponent.json");
}

#[test]
fn n_number_invalid_minus_utf_minus_8_minus_in_minus_int() {
    assert_reject("n_number_invalid-utf-8-in-int.json");
}

#[test]
fn n_number_minus_infinity() {
    assert_reject("n_number_minus_infinity.json");
}

#[test]
fn n_number_minus_sign_with_trailing_garbage() {
    assert_reject("n_number_minus_sign_with_trailing_garbage.json");
}

#[test]
fn n_number_minus_space_1() {
    assert_reject("n_number_minus_space_1.json");
}

#[test]
fn n_number_neg_int_starting_with_zero() {
    assert_reject("n_number_neg_int_starting_with_zero.json");
}

#[test]
fn n_number_neg_real_without_int_part() {
    assert_reject("n_number_neg_real_without_int_part.json");
}

#[test]
fn n_number_neg_with_garbage_at_end() {
    assert_reject("n_number_neg_with_garbage_at_end.json");
}

#[test]
fn n_number_real_garbage_after_e() {
    assert_reject("n_number_real_garbage_after_e.json");
}

#[test]
fn n_number_real_with_invalid_utf8_after_e() {
    assert_reject("n_number_real_with_invalid_utf8_after_e.json");
}

#[test]
fn n_number_real_without_fractional_part() {
    assert_reject("n_number_real_without_fractional_part.json");
}

#[test]
fn n_number_starting_with_dot() {
    assert_reject("n_number_starting_with_dot.json");
}

#[test]
fn n_number_with_alpha() {
    assert_reject("n_number_with_alpha.json");
}

#[test]
fn n_number_with_alpha_char() {
    assert_reject("n_number_with_alpha_char.json");
}

#[test]
fn n_number_with_leading_zero() {
    assert_reject("n_number_with_leading_zero.json");
}

#[test]
fn n_object_bad_value() {
    assert_reject("n_object_bad_value.json");
}

#[test]
fn n_object_bracket_key() {
    assert_reject("n_object_bracket_key.json");
}

#[test]
fn n_object_comma_instead_of_colon() {
    assert_reject("n_object_comma_instead_of_colon.json");
}

#[test]
fn n_object_double_colon() {
    assert_reject("n_object_double_colon.json");
}

#[test]
fn n_object_emoji() {
    assert_reject("n_object_emoji.json");
}

#[test]
fn n_object_garbage_at_end() {
    assert_reject("n_object_garbage_at_end.json");
}

#[test]
fn n_object_key_with_single_quotes() {
    assert_reject("n_object_key_with_single_quotes.json");
}

#[test]
fn n_object_lone_continuation_byte_in_key_and_trailing_comma() {
    assert_reject("n_object_lone_continuation_byte_in_key_and_trailing_comma.json");
}

#[test]
fn n_object_missing_colon() {
    assert_reject("n_object_missing_colon.json");
}

#[test]
fn n_object_missing_key() {
    assert_reject("n_object_missing_key.json");
}

#[test]
fn n_object_missing_semicolon() {
    assert_reject("n_object_missing_semicolon.json");
}

#[test]
fn n_object_missing_value() {
    assert_reject("n_object_missing_value.json");
}

#[test]
fn n_object_no_minus_colon() {
    assert_reject("n_object_no-colon.json");
}

#[test]
fn n_object_non_string_key() {
    assert_reject("n_object_non_string_key.json");
}

#[test]
fn n_object_non_string_key_but_huge_number_instead() {
    assert_reject("n_object_non_string_key_but_huge_number_instead.json");
}

#[test]
fn n_object_repeated_null_null() {
    assert_reject("n_object_repeated_null_null.json");
}

#[test]
fn n_object_several_trailing_commas() {
    assert_reject("n_object_several_trailing_commas.json");
}

#[test]
fn n_object_single_quote() {
    assert_reject("n_object_single_quote.json");
}

#[test]
fn n_object_trailing_comma() {
    assert_reject("n_object_trailing_comma.json");
}

#[test]
fn n_object_trailing_comment() {
    assert_reject("n_object_trailing_comment.json");
}

#[test]
fn n_object_trailing_comment_open() {
    assert_reject("n_object_trailing_comment_open.json");
}

#[test]
fn n_object_trailing_comment_slash_open() {
    assert_reject("n_object_trailing_comment_slash_open.json");
}

#[test]
fn n_object_trailing_comment_slash_open_incomplete() {
    assert_reject("n_object_trailing_comment_slash_open_incomplete.json");
}

#[test]
fn n_object_two_commas_in_a_row() {
    assert_reject("n_object_two_commas_in_a_row.json");
}

#[test]
fn n_object_unquoted_key() {
    assert_reject("n_object_unquoted_key.json");
}

#[test]
fn n_object_unterminated_minus_value() {
    assert_reject("n_object_unterminated-value.json");
}

#[test]
fn n_object_with_single_string() {
    assert_reject("n_object_with_single_string.json");
}

#[test]
fn n_object_with_trailing_garbage() {
    assert_reject("n_object_with_trailing_garbage.json");
}

#[test]
fn n_single_space() {
    assert_reject("n_single_space.json");
}

#[test]
fn n_string_1_surrogate_then_escape() {
    assert_reject("n_string_1_surrogate_then_escape.json");
}

#[test]
fn n_string_1_surrogate_then_escape_u() {
    assert_reject("n_string_1_surrogate_then_escape_u.json");
}

#[test]
fn n_string_1_surrogate_then_escape_u1() {
    assert_reject("n_string_1_surrogate_then_escape_u1.json");
}

#[test]
fn n_string_1_surrogate_then_escape_u1x() {
    assert_reject("n_string_1_surrogate_then_escape_u1x.json");
}

#[test]
fn n_string_accentuated_char_no_quotes() {
    assert_reject("n_string_accentuated_char_no_quotes.json");
}

#[test]
fn n_string_backslash_00() {
    assert_reject("n_string_backslash_00.json");
}

#[test]
fn n_string_escape_x() {
    assert_reject("n_string_escape_x.json");
}

#[test]
fn n_string_escaped_backslash_bad() {
    assert_reject("n_string_escaped_backslash_bad.json");
}

#[test]
fn n_string_escaped_ctrl_char_tab() {
    assert_reject("n_string_escaped_ctrl_char_tab.json");
}

#[test]
fn n_string_escaped_emoji() {
    assert_reject("n_string_escaped_emoji.json");
}

#[test]
fn n_string_incomplete_escape() {
    assert_reject("n_string_incomplete_escape.json");
}

#[test]
fn n_string_incomplete_escaped_character() {
    assert_reject("n_string_incomplete_escaped_character.json");
}

#[test]
fn n_string_incomplete_surrogate() {
    assert_reject("n_string_incomplete_surrogate.json");
}

#[test]
fn n_string_incomplete_surrogate_escape_invalid() {
    assert_reject("n_string_incomplete_surrogate_escape_invalid.json");
}

#[test]
fn n_string_invalid_minus_utf_minus_8_minus_in_minus_escape() {
    assert_reject("n_string_invalid-utf-8-in-escape.json");
}

#[test]
fn n_string_invalid_backslash_esc() {
    assert_reject("n_string_invalid_backslash_esc.json");
}

#[test]
fn n_string_invalid_unicode_escape() {
    assert_reject("n_string_invalid_unicode_escape.json");
}

#[test]
fn n_string_invalid_utf8_after_escape() {
    assert_reject("n_string_invalid_utf8_after_escape.json");
}

#[test]
fn n_string_leading_uescaped_thinspace() {
    assert_reject("n_string_leading_uescaped_thinspace.json");
}

#[test]
fn n_string_no_quotes_with_bad_escape() {
    assert_reject("n_string_no_quotes_with_bad_escape.json");
}

#[test]
fn n_string_single_doublequote() {
    assert_reject("n_string_single_doublequote.json");
}

#[test]
fn n_string_single_quote() {
    assert_reject("n_string_single_quote.json");
}

#[test]
fn n_string_single_string_no_double_quotes() {
    assert_reject("n_string_single_string_no_double_quotes.json");
}

#[test]
fn n_string_start_escape_unclosed() {
    assert_reject("n_string_start_escape_unclosed.json");
}

#[test]
fn n_string_unescaped_ctrl_char() {
    assert_reject("n_string_unescaped_ctrl_char.json");
}

#[test]
fn n_string_unescaped_newline() {
    assert_reject("n_string_unescaped_newline.json");
}

#[test]
fn n_string_unescaped_tab() {
    assert_reject("n_string_unescaped_tab.json");
}

#[test]
fn n_string_unicode_CapitalU() {
    assert_reject("n_string_unicode_CapitalU.json");
}

#[test]
fn n_string_with_trailing_garbage() {
    assert_reject("n_string_with_trailing_garbage.json");
}

#[test]
fn n_structure_U_plus_2060_word_joined() {
    assert_reject("n_structure_U+2060_word_joined.json");
}

#[test]
fn n_structure_UTF8_BOM_no_data() {
    assert_reject("n_structure_UTF8_BOM_no_data.json");
}

#[test]
fn n_structure_angle_bracket() {
    assert_reject("n_structure_angle_bracket_..json");
}

#[test]
fn n_structure_angle_bracket_null() {
    assert_reject("n_structure_angle_bracket_null.json");
}

#[test]
fn n_structure_array_trailing_garbage() {
    assert_reject("n_structure_array_trailing_garbage.json");
}

#[test]
fn n_structure_array_with_extra_array_close() {
    assert_reject("n_structure_array_with_extra_array_close.json");
}

#[test]
fn n_structure_array_with_unclosed_string() {
    assert_reject("n_structure_array_with_unclosed_string.json");
}

#[test]
fn n_structure_ascii_minus_unicode_minus_identifier() {
    assert_reject("n_structure_ascii-unicode-identifier.json");
}

#[test]
fn n_structure_capitalized_True() {
    assert_reject("n_structure_capitalized_True.json");
}

#[test]
fn n_structure_close_unopened_array() {
    assert_reject("n_structure_close_unopened_array.json");
}

#[test]
fn n_structure_comma_instead_of_closing_brace() {
    assert_reject("n_structure_comma_instead_of_closing_brace.json");
}

#[test]
fn n_structure_double_array() {
    assert_reject("n_structure_double_array.json");
}

#[test]
fn n_structure_end_array() {
    assert_reject("n_structure_end_array.json");
}

#[test]
fn n_structure_incomplete_UTF8_BOM() {
    assert_reject("n_structure_incomplete_UTF8_BOM.json");
}

#[test]
fn n_structure_lone_minus_invalid_minus_utf_minus_8() {
    assert_reject("n_structure_lone-invalid-utf-8.json");
}

#[test]
fn n_structure_lone_minus_open_minus_bracket() {
    assert_reject("n_structure_lone-open-bracket.json");
}

#[test]
fn n_structure_no_data() {
    assert_reject("n_structure_no_data.json");
}

#[test]
fn n_structure_null_minus_byte_minus_outside_minus_string() {
    assert_reject("n_structure_null-byte-outside-string.json");
}

#[test]
fn n_structure_number_with_trailing_garbage() {
    assert_reject("n_structure_number_with_trailing_garbage.json");
}

#[test]
fn n_structure_object_followed_by_closing_object() {
    assert_reject("n_structure_object_followed_by_closing_object.json");
}

#[test]
fn n_structure_object_unclosed_no_value() {
    assert_reject("n_structure_object_unclosed_no_value.json");
}

#[test]
fn n_structure_object_with_comment() {
    assert_reject("n_structure_object_with_comment.json");
}

#[test]
fn n_structure_object_with_trailing_garbage() {
    assert_reject("n_structure_object_with_trailing_garbage.json");
}

#[test]
fn n_structure_open_array_apostrophe() {
    assert_reject("n_structure_open_array_apostrophe.json");
}

#[test]
fn n_structure_open_array_comma() {
    assert_reject("n_structure_open_array_comma.json");
}

#[test]
fn n_structure_open_array_open_object() {
    assert_reject("n_structure_open_array_open_object.json");
}

#[test]
fn n_structure_open_array_open_string() {
    assert_reject("n_structure_open_array_open_string.json");
}

#[test]
fn n_structure_open_array_string() {
    assert_reject("n_structure_open_array_string.json");
}

#[test]
fn n_structure_open_object() {
    assert_reject("n_structure_open_object.json");
}

#[test]
fn n_structure_open_object_close_array() {
    assert_reject("n_structure_open_object_close_array.json");
}

#[test]
fn n_structure_open_object_comma() {
    assert_reject("n_structure_open_object_comma.json");
}

#[test]
fn n_structure_open_object_open_array() {
    assert_reject("n_structure_open_object_open_array.json");
}

#[test]
fn n_structure_open_object_open_string() {
    assert_reject("n_structure_open_object_open_string.json");
}

#[test]
fn n_structure_open_object_string_with_apostrophes() {
    assert_reject("n_structure_open_object_string_with_apostrophes.json");
}

#[test]
fn n_structure_open_open() {
    assert_reject("n_structure_open_open.json");
}

#[test]
fn n_structure_single_eacute() {
    assert_reject("n_structure_single_eacute.json");
}

#[test]
fn n_structure_single_star() {
    assert_reject("n_structure_single_star.json");
}

#[test]
fn n_structure_trailing() {
    assert_reject("n_structure_trailing_#.json");
}

#[test]
fn n_structure_uescaped_LF_before_string() {
    assert_reject("n_structure_uescaped_LF_before_string.json");
}

#[test]
fn n_structure_unclosed_array() {
    assert_reject("n_structure_unclosed_array.json");
}

#[test]
fn n_structure_unclosed_array_partial_null() {
    assert_reject("n_structure_unclosed_array_partial_null.json");
}

#[test]
fn n_structure_unclosed_array_unfinished_false() {
    assert_reject("n_structure_unclosed_array_unfinished_false.json");
}

#[test]
fn n_structure_unclosed_array_unfinished_true() {
    assert_reject("n_structure_unclosed_array_unfinished_true.json");
}

#[test]
fn n_structure_unclosed_object() {
    assert_reject("n_structure_unclosed_object.json");
}

#[test]
fn n_structure_unicode_minus_identifier() {
    assert_reject("n_structure_unicode-identifier.json");
}

#[test]
fn n_structure_whitespace_U_plus_2060_word_joiner() {
    assert_reject("n_structure_whitespace_U+2060_word_joiner.json");
}

#[test]
fn n_structure_whitespace_formfeed() {
    assert_reject("n_structure_whitespace_formfeed.json");
}

// --- implementation-defined (i_) ---
#[test]
fn i_number_double_huge_neg_exp() {
    assert_no_panic("i_number_double_huge_neg_exp.json");
}

#[test]
fn i_number_huge_exp() {
    assert_no_panic("i_number_huge_exp.json");
}

#[test]
fn i_number_neg_int_huge_exp() {
    assert_no_panic("i_number_neg_int_huge_exp.json");
}

#[test]
fn i_number_pos_double_huge_exp() {
    assert_no_panic("i_number_pos_double_huge_exp.json");
}

#[test]
fn i_number_real_neg_overflow() {
    assert_no_panic("i_number_real_neg_overflow.json");
}

#[test]
fn i_number_real_pos_overflow() {
    assert_no_panic("i_number_real_pos_overflow.json");
}

#[test]
fn i_number_real_underflow() {
    assert_no_panic("i_number_real_underflow.json");
}

#[test]
fn i_number_too_big_neg_int() {
    assert_no_panic("i_number_too_big_neg_int.json");
}

#[test]
fn i_number_too_big_pos_int() {
    assert_no_panic("i_number_too_big_pos_int.json");
}

#[test]
fn i_number_very_big_negative_int() {
    assert_no_panic("i_number_very_big_negative_int.json");
}

#[test]
fn i_object_key_lone_2nd_surrogate() {
    assert_no_panic("i_object_key_lone_2nd_surrogate.json");
}

#[test]
fn i_string_1st_surrogate_but_2nd_missing() {
    assert_no_panic("i_string_1st_surrogate_but_2nd_missing.json");
}

#[test]
fn i_string_1st_valid_surrogate_2nd_invalid() {
    assert_no_panic("i_string_1st_valid_surrogate_2nd_invalid.json");
}

#[test]
fn i_string_UTF_minus_16LE_with_BOM() {
    assert_no_panic("i_string_UTF-16LE_with_BOM.json");
}

#[test]
fn i_string_UTF_minus_8_invalid_sequence() {
    assert_no_panic("i_string_UTF-8_invalid_sequence.json");
}

#[test]
fn i_string_UTF8_surrogate_U_plus_D800() {
    assert_no_panic("i_string_UTF8_surrogate_U+D800.json");
}

#[test]
fn i_string_incomplete_surrogate_and_escape_valid() {
    assert_no_panic("i_string_incomplete_surrogate_and_escape_valid.json");
}

#[test]
fn i_string_incomplete_surrogate_pair() {
    assert_no_panic("i_string_incomplete_surrogate_pair.json");
}

#[test]
fn i_string_incomplete_surrogates_escape_valid() {
    assert_no_panic("i_string_incomplete_surrogates_escape_valid.json");
}

#[test]
fn i_string_invalid_lonely_surrogate() {
    assert_no_panic("i_string_invalid_lonely_surrogate.json");
}

#[test]
fn i_string_invalid_surrogate() {
    assert_no_panic("i_string_invalid_surrogate.json");
}

#[test]
fn i_string_invalid_utf_minus_8() {
    assert_no_panic("i_string_invalid_utf-8.json");
}

#[test]
fn i_string_inverted_surrogates_U_plus_1D11E() {
    assert_no_panic("i_string_inverted_surrogates_U+1D11E.json");
}

#[test]
fn i_string_iso_latin_1() {
    assert_no_panic("i_string_iso_latin_1.json");
}

#[test]
fn i_string_lone_second_surrogate() {
    assert_no_panic("i_string_lone_second_surrogate.json");
}

#[test]
fn i_string_lone_utf8_continuation_byte() {
    assert_no_panic("i_string_lone_utf8_continuation_byte.json");
}

#[test]
fn i_string_not_in_unicode_range() {
    assert_no_panic("i_string_not_in_unicode_range.json");
}

#[test]
fn i_string_overlong_sequence_2_bytes() {
    assert_no_panic("i_string_overlong_sequence_2_bytes.json");
}

#[test]
fn i_string_overlong_sequence_6_bytes() {
    assert_no_panic("i_string_overlong_sequence_6_bytes.json");
}

#[test]
fn i_string_overlong_sequence_6_bytes_null() {
    assert_no_panic("i_string_overlong_sequence_6_bytes_null.json");
}

#[test]
fn i_string_truncated_minus_utf_minus_8() {
    assert_no_panic("i_string_truncated-utf-8.json");
}

#[test]
fn i_string_utf16BE_no_BOM() {
    assert_no_panic("i_string_utf16BE_no_BOM.json");
}

#[test]
fn i_string_utf16LE_no_BOM() {
    assert_no_panic("i_string_utf16LE_no_BOM.json");
}

#[test]
fn i_structure_UTF_minus_8_BOM_empty_object() {
    assert_no_panic("i_structure_UTF-8_BOM_empty_object.json");
}
