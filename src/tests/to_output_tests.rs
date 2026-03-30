
#[cfg(test)]
mod test_to_output_tests {
    use crate::ToOutput;

    // --- String & str ---

    #[test]
    fn string_passthrough() {
        assert_eq!("hello".to_owned().to_output(), "hello");
    }

    #[test]
    fn str_to_output() {
        assert_eq!("hello".to_output(), "hello");
    }

    #[test]
    fn empty_string() {
        assert_eq!(String::new().to_output(), "");
    }

    // --- char ---

    #[test]
    fn char_single() {
        assert_eq!('a'.to_output(), "a");
    }

    #[test]
    fn char_unicode() {
        assert_eq!('\u{e9}'.to_output(), "\u{e9}");
    }

    // --- bool ---

    #[test]
    fn bool_true() {
        assert_eq!(true.to_output(), "1");
    }

    #[test]
    fn bool_false() {
        assert_eq!(false.to_output(), "0");
    }

    // --- signed integers ---

    #[test]
    fn i8_positive() {
        assert_eq!(42_i8.to_output(), "42");
    }

    #[test]
    fn i8_negative() {
        assert_eq!((-1_i8).to_output(), "-1");
    }

    #[test]
    fn i32_zero() {
        assert_eq!(0_i32.to_output(), "0");
    }

    #[test]
    fn i64_large() {
        assert_eq!(1_000_000_000_000_i64.to_output(), "1000000000000");
    }

    #[test]
    fn i128_boundaries() {
        assert_eq!(i128::MAX.to_output(), i128::MAX.to_string());
        assert_eq!(i128::MIN.to_output(), i128::MIN.to_string());
    }

    // --- unsigned integers ---

    #[test]
    fn u32_value() {
        assert_eq!(42_u32.to_output(), "42");
    }

    #[test]
    fn u64_max() {
        assert_eq!(u64::MAX.to_output(), u64::MAX.to_string());
    }

    #[test]
    fn usize_value() {
        assert_eq!(100_usize.to_output(), "100");
    }

    // --- floats ---

    #[test]
    fn f32_value() {
        assert_eq!(1.5_f32.to_output(), "1.5");
    }

    #[test]
    fn f64_value() {
        assert_eq!(2.75_f64.to_output(), "2.75");
    }

    #[test]
    fn f64_negative() {
        assert_eq!((-0.5_f64).to_output(), "-0.5");
    }

    // --- Vec ---

    #[test]
    fn vec_empty() {
        let v: Vec<i32> = vec![];
        assert_eq!(v.to_output(), "");
    }

    #[test]
    fn vec_single_element() {
        assert_eq!(vec![42].to_output(), "42\n");
    }

    #[test]
    fn vec_integers() {
        assert_eq!(vec![1, 2, 3].to_output(), "1 2 3\n");
    }

    #[test]
    fn vec_strings_space_separated() {
        assert_eq!(
            vec!["hello".to_owned(), "world".to_owned()].to_output(),
            "hello world\n"
        );
    }

    #[test]
    fn vec_str_refs() {
        assert_eq!(vec!["a", "b", "c"].to_output(), "a b c\n");
    }

    #[test]
    fn vec_all_elements_end_with_newline() {
        let v = vec!["line1\n".to_owned(), "line2\n".to_owned()];
        assert_eq!(v.to_output(), "line1\nline2\n");
    }

    #[test]
    fn vec_mixed_newlines() {
        // "a\n" ends with newline so no space appended; "b" doesn't so space; "c\n" ends with newline
        let v = vec!["a\n", "b", "c\n"];
        assert_eq!(v.to_output(), "a\nb c\n");
    }

    #[test]
    fn vec_nested_vecs() {
        // Vec<Vec<i32>>: inner vecs produce "1 2\n" and "3 4\n" (ending in \n)
        let v = vec![vec![1, 2], vec![3, 4]];
        assert_eq!(v.to_output(), "1 2\n3 4\n");
    }

    #[test]
    fn vec_bools() {
        assert_eq!(vec![true, false, true].to_output(), "1 0 1\n");
    }

    // --- derive(ToOutput) ---

    #[derive(ToOutput)]
    struct MyStruct {
        a: i32,
        b: String,
        c: Vec<i32>,
    }

    #[test]
    fn derive_struct_named() {
        let s = MyStruct {
            a: 42,
            b: "hello".to_owned(),
            c: vec![1, 2, 3],
        };
        assert_eq!(s.to_output(), "42\nhello\n1 2 3\n\n");
    }

    #[derive(ToOutput)]
    struct MyTupleStruct(i32, String, Vec<i32>);

    #[test]
    fn derive_struct_tuple() {
        let s = MyTupleStruct(42, "hello".to_owned(), vec![1, 2, 3]);
        assert_eq!(s.to_output(), "42\nhello\n1 2 3\n\n");
    }

    #[derive(ToOutput)]
    struct UnitStruct;

    #[test]
    fn derive_struct_unit() {
        let s = UnitStruct;
        assert_eq!(s.to_output(), "");
    }
}
