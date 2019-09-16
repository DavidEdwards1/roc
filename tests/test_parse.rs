#[macro_use]
extern crate pretty_assertions;
#[macro_use]
extern crate indoc;
extern crate bumpalo;
extern crate combine; // OBSOLETE
extern crate roc;

extern crate quickcheck;

#[macro_use(quickcheck)]
extern crate quickcheck_macros;

mod helpers;

#[cfg(test)]
mod test_parser {
    use bumpalo::Bump;
    use helpers::located;
    use roc::parse;
    use roc::parse::ast::Attempting;
    use roc::parse::ast::Expr::{self, *};
    use roc::parse::parser::{Fail, FailReason, Parser, State};
    use roc::parse::problems::Problem;
    use roc::region::{Located, Region};
    use std::{f64, i64};

    fn parse_with<'a>(arena: &'a Bump, input: &'a str) -> Result<Expr<'a>, Fail> {
        let state = State::new(&input, Attempting::Module);
        let parser = parse::expr();
        let answer = parser.parse(&arena, state);

        answer.map(|(expr, _)| expr).map_err(|(fail, _)| fail)
    }

    fn assert_parses_to<'a>(input: &'a str, expected_expr: Expr<'a>) {
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(Ok(expected_expr), actual);
    }

    fn assert_parsing_fails<'a>(input: &'a str, reason: FailReason, attempting: Attempting) {
        let arena = Bump::new();
        let actual = parse_with(&arena, input);
        let expected_fail = Fail { reason, attempting };

        assert_eq!(Err(expected_fail), actual);
    }

    fn assert_malformed_str<'a>(input: &'a str, expected_probs: Vec<Located<Problem>>) {
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(
            Ok(Expr::MalformedStr(expected_probs.into_boxed_slice())),
            actual
        );
    }

    // STRING LITERALS

    fn expect_parsed_str(input: &str, expected: &str) {
        assert_parses_to(expected, Str(input.into()));
    }

    #[test]
    fn empty_string() {
        assert_parses_to(
            indoc!(
                r#"
                ""
                "#
            ),
            EmptyStr,
        );
    }

    #[test]
    fn one_char_list() {
        assert_parses_to(
            indoc!(
                r#"
                "x"
                "#
            ),
            Str("x".into()),
        );
    }

    #[test]
    fn multi_char_list() {
        assert_parses_to(
            indoc!(
                r#"
                "foo"
                "#
            ),
            Str("foo".into()),
        );
    }

    #[test]
    fn string_without_escape() {
        expect_parsed_str("a", r#""a""#);
        expect_parsed_str("ab", r#""ab""#);
        expect_parsed_str("abc", r#""abc""#);
        expect_parsed_str("123", r#""123""#);
        expect_parsed_str("abc123", r#""abc123""#);
        expect_parsed_str("123abc", r#""123abc""#);
        expect_parsed_str("123 abc 456 def", r#""123 abc 456 def""#);
    }

    #[test]
    fn string_with_special_escapes() {
        expect_parsed_str(r#"x\x"#, r#""x\\x""#);
        expect_parsed_str(r#"x"x"#, r#""x\"x""#);
        expect_parsed_str("x\tx", r#""x\tx""#);
        expect_parsed_str("x\rx", r#""x\rx""#);
        expect_parsed_str("x\nx", r#""x\nx""#);
    }

    #[test]
    fn string_with_escaped_interpolation() {
        assert_parses_to(
            // This should NOT be string interpolation, because of the \\
            indoc!(
                r#"
                "abcd\\(efg)hij"
                "#
            ),
            Str(r#"abcd\(efg)hij"#.into()),
        );
    }

    #[test]
    fn string_with_single_quote() {
        // This shoud NOT be escaped in a string.
        expect_parsed_str("x'x", r#""x'x""#);
    }

    #[test]
    fn string_with_valid_unicode_escapes() {
        expect_parsed_str("x\u{00A0}x", r#""x\u{00A0}x""#);
        expect_parsed_str("x\u{101010}x", r#""x\u{101010}x""#);
    }

    #[test]
    fn string_with_too_large_unicode_escape() {
        // Should be too big - max size should be 10FFFF.
        // (Rust has this restriction. I assume it's a good idea.)
        assert_malformed_str(
            r#""abc\u{110000}def""#,
            vec![located(0, 7, 0, 12, Problem::UnicodeCodePointTooLarge)],
        );
    }

    #[test]
    fn string_with_no_unicode_digits() {
        // No digits specified
        assert_malformed_str(
            r#""blah\u{}foo""#,
            vec![located(0, 5, 0, 8, Problem::NoUnicodeDigits)],
        );
    }

    #[test]
    fn string_with_no_unicode_opening_brace() {
        // No opening curly brace. It can't be sure if the closing brace
        // was intended to be a closing brace for the unicode escape, so
        // report that there were no digits specified.
        assert_malformed_str(
            r#""abc\u00A0}def""#,
            vec![located(0, 4, 0, 5, Problem::NoUnicodeDigits)],
        );
    }

    #[test]
    fn string_with_no_unicode_closing_brace() {
        // No closing curly brace
        assert_malformed_str(
            r#""blah\u{stuff""#,
            vec![located(0, 5, 0, 12, Problem::MalformedEscapedUnicode)],
        );
    }

    #[test]
    fn string_with_no_unicode_braces() {
        // No curly braces
        assert_malformed_str(
            r#""zzzz\uzzzzz""#,
            vec![located(0, 5, 0, 6, Problem::NoUnicodeDigits)],
        );
    }

    #[test]
    fn string_with_interpolation_at_start() {
        let input = indoc!(
            r#"
                 "\(abc)defg"
                 "#
        );
        let (args, ret) = (vec![("", located(0, 2, 0, 4, Var("abc")))], "defg");
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(
            Ok(InterpolatedStr(&(arena.alloc_slice_clone(&args), ret))),
            actual
        );
    }

    #[test]
    fn string_with_interpolation_at_end() {
        let input = indoc!(
            r#"
                 "abcd\(efg)"
                 "#
        );
        let (args, ret) = (vec![("abcd", located(0, 6, 0, 8, Var("efg")))], "");
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(
            Ok(InterpolatedStr(&(arena.alloc_slice_clone(&args), ret))),
            actual
        );
    }

    #[test]
    fn string_with_interpolation_in_middle() {
        let input = indoc!(
            r#"
                 "abc\(defg)hij"
                 "#
        );
        let (args, ret) = (vec![("abc", located(0, 5, 0, 8, Var("defg")))], "hij");
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(
            Ok(InterpolatedStr(&(arena.alloc_slice_clone(&args), ret))),
            actual
        );
    }

    #[test]
    fn string_with_two_interpolations_in_middle() {
        let input = indoc!(
            r#"
                 "abc\(defg)hi\(jkl)mn"
                 "#
        );
        let (args, ret) = (
            vec![
                ("abc", located(0, 5, 0, 8, Var("defg"))),
                ("hi", located(0, 14, 0, 16, Var("jkl"))),
            ],
            "mn",
        );
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(
            Ok(InterpolatedStr(&(arena.alloc_slice_clone(&args), ret))),
            actual
        );
    }

    #[test]
    fn string_with_four_interpolations() {
        let input = indoc!(
            r#"
                 "\(abc)def\(ghi)jkl\(mno)pqrs\(tuv)"
                 "#
        );
        let (args, ret) = (
            vec![
                ("", located(0, 2, 0, 4, Var("abc"))),
                ("def", located(0, 11, 0, 13, Var("ghi"))),
                ("jkl", located(0, 20, 0, 22, Var("mno"))),
                ("pqrs", located(0, 30, 0, 32, Var("tuv"))),
            ],
            "",
        );
        let arena = Bump::new();
        let actual = parse_with(&arena, input);

        assert_eq!(
            Ok(InterpolatedStr(&(arena.alloc_slice_clone(&args), ret))),
            actual
        );
    }

    #[test]
    fn empty_source_file() {
        assert_parsing_fails("", FailReason::Eof(Region::zero()), Attempting::Expression);
    }

    #[test]
    fn first_line_too_long() {
        let max_line_length = std::u16::MAX as usize;

        // the string literal "ZZZZZZZZZ" but with way more Zs
        let too_long_str_body: String = (1..max_line_length)
            .into_iter()
            .map(|_| "Z".to_string())
            .collect();
        let too_long_str = format!("\"{}\"", too_long_str_body);

        // Make sure it's longer than our maximum line length
        assert_eq!(too_long_str.len(), max_line_length + 1);

        assert_parsing_fails(
            &too_long_str,
            FailReason::LineTooLong(0),
            Attempting::Expression,
        );
    }

    // INT LITERALS

    #[test]
    fn zero_int() {
        assert_parses_to("0", Int(0));
    }

    #[test]
    fn positive_int() {
        assert_parses_to("1", Int(1));
        assert_parses_to("42", Int(42));
    }

    #[test]
    fn negative_int() {
        assert_parses_to("-1", Int(-1));
        assert_parses_to("-42", Int(-42));
    }

    #[test]
    fn highest_int() {
        assert_parses_to(i64::MAX.to_string().as_str(), Int(i64::MAX));
    }

    #[test]
    fn lowest_int() {
        assert_parses_to(i64::MIN.to_string().as_str(), Int(i64::MIN));
    }

    #[test]
    fn int_with_underscore() {
        assert_parses_to("1_2_34_567", Int(1234567));
        assert_parses_to("-1_2_34_567", Int(-1234567));
        // The following cases are silly. They aren't supported on purpose,
        // but there would be a performance cost to explicitly disallowing them,
        // which doesn't seem like it would benefit anyone.
        assert_parses_to("1_", Int(1));
        assert_parses_to("1__23", Int(123));
    }

    #[quickcheck]
    fn all_i64_values_parse(num: i64) {
        assert_parses_to(num.to_string().as_str(), Int(num));
    }

    #[test]
    fn int_too_large() {
        assert_parses_to(
            (i64::MAX as i128 + 1).to_string().as_str(),
            MalformedInt(Problem::OutsideSupportedRange),
        );
    }

    #[test]
    fn int_too_small() {
        assert_parses_to(
            (i64::MIN as i128 - 1).to_string().as_str(),
            MalformedInt(Problem::OutsideSupportedRange),
        );
    }

    // FLOAT LITERALS

    #[test]
    fn zero_float() {
        assert_parses_to("0.0", Float(0.0));
    }

    #[test]
    fn positive_float() {
        assert_parses_to("1.0", Float(1.0));
        assert_parses_to("1.1", Float(1.1));
        assert_parses_to("42.0", Float(42.0));
        assert_parses_to("42.9", Float(42.9));
    }

    #[test]
    fn highest_float() {
        assert_parses_to(&format!("{}.0", f64::MAX), Float(f64::MAX));
    }

    #[test]
    fn negative_float() {
        assert_parses_to("-1.0", Float(-1.0));
        assert_parses_to("-1.1", Float(-1.1));
        assert_parses_to("-42.0", Float(-42.0));
        assert_parses_to("-42.9", Float(-42.9));
    }

    #[test]
    fn lowest_float() {
        assert_parses_to(&format!("{}.0", f64::MIN), Float(f64::MIN));
    }

    #[test]
    fn float_with_underscores() {
        assert_parses_to("1_23_456.0_1_23_456", Float(123456.0123456));
        assert_parses_to("-1_23_456.0_1_23_456", Float(-123456.0123456));
    }

    #[quickcheck]
    fn all_f64_values_parse(num: f64) {
        assert_parses_to(num.to_string().as_str(), Float(num));
    }

    #[test]
    fn float_too_large() {
        assert_parses_to(
            format!("{}1.0", f64::MAX).as_str(),
            MalformedFloat(Problem::OutsideSupportedRange),
        );
    }

    #[test]
    fn float_too_small() {
        assert_parses_to(
            format!("{}1.0", f64::MIN).as_str(),
            MalformedFloat(Problem::OutsideSupportedRange),
        );
    }

    // TODO test what happens when interpolated strings contain 1+ malformed idents
    //
    // TODO test for \t \r and \n in string literals *outside* unicode escape sequence!
    //
    // TODO verify that when a string literal contains a newline before the
    // closing " it correctly updates both the line *and* column in the State.
}
