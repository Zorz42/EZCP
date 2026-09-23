#[cfg(test)]
mod test_to_output_tests {
    use crate::ToOutput;

    #[test]
    fn scalars() {
        assert_eq!("hello".to_owned().to_output(), "hello");
        assert_eq!("hello".to_output(), "hello");
        assert_eq!(String::new().to_output(), "");
        assert_eq!('a'.to_output(), "a");
        assert_eq!('\u{e9}'.to_output(), "\u{e9}");
        assert_eq!(true.to_output(), "1");
        assert_eq!(false.to_output(), "0");
        assert_eq!(42_i8.to_output(), "42");
        assert_eq!((-1_i8).to_output(), "-1");
        assert_eq!(0_i32.to_output(), "0");
        assert_eq!(1_000_000_000_000_i64.to_output(), "1000000000000");
        assert_eq!(i128::MAX.to_output(), i128::MAX.to_string());
        assert_eq!(i128::MIN.to_output(), i128::MIN.to_string());
        assert_eq!(42_u32.to_output(), "42");
        assert_eq!(u64::MAX.to_output(), u64::MAX.to_string());
        assert_eq!(100_usize.to_output(), "100");
        assert_eq!(1.5_f32.to_output(), "1.5");
        assert_eq!(2.75_f64.to_output(), "2.75");
        assert_eq!((-0.5_f64).to_output(), "-0.5");
    }

    #[test]
    fn vecs() {
        assert_eq!(Vec::<i32>::new().to_output(), "");
        assert_eq!(vec![42].to_output(), "42\n");
        assert_eq!(vec![1, 2, 3].to_output(), "1 2 3\n");
        assert_eq!(vec!["hello".to_owned(), "world".to_owned()].to_output(), "hello world\n");
        assert_eq!(vec!["a", "b", "c"].to_output(), "a b c\n");
        assert_eq!(vec!["line1\n", "line2\n"].to_output(), "line1\nline2\n");
        assert_eq!(vec!["a\n", "b", "c\n"].to_output(), "a\nb c\n");
        assert_eq!(vec![vec![1, 2], vec![3, 4]].to_output(), "1 2\n3 4\n");
        assert_eq!(vec![true, false, true].to_output(), "1 0 1\n");
    }

    #[test]
    fn tuples() {
        assert_eq!((42,).to_output(), "42\n");
        assert_eq!((1, 2).to_output(), "1 2\n");
        assert_eq!((1_i32, "hello", true).to_output(), "1 hello 1\n");
        assert_eq!(("a\n", "b\n", "c").to_output(), "a\nb\nc\n");
        assert_eq!((1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12).to_output(), "1 2 3 4 5 6 7 8 9 10 11 12\n");
        // A field that already ends a line does not get a second newline.
        assert_eq!((1, "x\n").to_output(), "1 x\n");
        assert_eq!(("a\n",).to_output(), "a\n");
        assert_eq!((vec![1, 2], vec![3, 4]).to_output(), "1 2\n3 4\n");
    }

    #[derive(ToOutput)]
    struct Named {
        a: i32,
        b: String,
        c: Vec<i32>,
    }

    #[derive(ToOutput)]
    struct Tuple(i32, String, Vec<i32>);

    #[derive(ToOutput)]
    struct Unit;

    #[derive(ToOutput)]
    struct WithEmpty {
        first: Vec<i32>,
        middle: i32,
        last: Vec<i32>,
    }

    #[test]
    fn derived() {
        assert_eq!(
            Named {
                a: 42,
                b: "hello".to_owned(),
                c: vec![1, 2, 3]
            }
            .to_output(),
            "42\nhello\n1 2 3\n"
        );
        assert_eq!(Tuple(42, "hello".to_owned(), vec![1, 2, 3]).to_output(), "42\nhello\n1 2 3\n");
        assert_eq!(Unit.to_output(), "");
        assert_eq!(
            WithEmpty {
                first: vec![],
                middle: 7,
                last: vec![]
            }
            .to_output(),
            "7\n"
        );
        assert_eq!(
            WithEmpty {
                first: vec![1],
                middle: 7,
                last: vec![2]
            }
            .to_output(),
            "1\n7\n2\n"
        );
    }
}

/// Without the trait in scope, as with `#[derive(ezcp::ToOutput)]` elsewhere.
#[cfg(test)]
mod derive_without_the_trait_in_scope {
    #[derive(crate::ToOutput)]
    struct Input {
        n: usize,
        values: Vec<i32>,
    }

    #[test]
    fn derive_works_without_importing_the_trait() {
        assert_eq!(crate::ToOutput::to_output(Input { n: 2, values: vec![1, 2] }), "2\n1 2\n");
    }
}
