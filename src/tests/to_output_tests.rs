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

/// The rules `ToOutput` followed before values were written into a shared
/// buffer, applied to fields that are already rendered. Tests are identified
/// by their bytes, so the buffer must not change a single one.
#[cfg(test)]
mod same_bytes_as_before {
    use crate::rng::Rng;
    use crate::{Counted, CountedLines, Lines, ToOutput};

    fn old_vec(fields: &[String]) -> String {
        let mut res = String::new();
        for field in fields {
            res.push_str(field);
            if !field.ends_with('\n') {
                res.push(' ');
            }
        }
        if res.ends_with(' ') {
            res.pop();
            res.push('\n');
        }
        res
    }

    fn old_tuple(fields: &[String]) -> String {
        let mut res = String::new();
        for field in fields {
            res.push_str(field);
            if !field.ends_with(char::is_whitespace) {
                res.push(' ');
            }
        }
        if res.ends_with(' ') {
            res.pop();
        }
        if !res.ends_with('\n') {
            res.push('\n');
        }
        res
    }

    fn old_struct(fields: &[String]) -> String {
        let mut res = String::new();
        for field in fields.iter().filter(|field| !field.is_empty()) {
            res.push_str(field);
            if !res.ends_with('\n') {
                res.push('\n');
            }
        }
        res
    }

    #[derive(ToOutput, Clone)]
    struct Record {
        text: String,
        list: Vec<String>,
        pair: (String, i64),
    }

    /// Text whose whitespace is as awkward as a hand-written generator's can be.
    fn text(rng: &mut Rng) -> String {
        let pieces = ["", "", "a", "7", " ", "\n", "x y", "b\n", "c ", "\t", " d", "\n\n", "\u{e9}", "e\r\n"];
        (0..rng.random_range(0..3)).map(|_| *rng.choose(&pieces).unwrap_or(&"")).collect()
    }

    fn texts(rng: &mut Rng) -> Vec<String> {
        (0..rng.random_range(0..4)).map(|_| text(rng)).collect()
    }

    /// Also checks that appending to a buffer gives the same bytes whatever
    /// the buffer already ends in.
    fn assert_renders<T: ToOutput + Clone>(value: &T, expected: &str) {
        assert_eq!(value.clone().to_output(), expected);
        for prefix in ["", "x", "x ", "x\n", " "] {
            let mut out = prefix.to_owned();
            value.clone().write_output(&mut out);
            assert_eq!(out, format!("{prefix}{expected}"), "after {prefix:?}");
        }
    }

    #[test]
    fn collections_tuples_and_structs() {
        let mut rng = Rng::from_seed(2024);
        for _ in 0..3000 {
            let (a, b, c) = (text(&mut rng), text(&mut rng), text(&mut rng));
            let (list, other) = (texts(&mut rng), texts(&mut rng));
            let number = rng.random_range(-1000..=1000_i64);

            assert_renders(&list, &old_vec(&list));
            assert_renders(&list.as_slice(), &old_vec(&list));
            assert_renders(&[a.clone(), b.clone()], &old_vec(&[a.clone(), b.clone()]));
            assert_renders(&(a.clone(),), &old_tuple(std::slice::from_ref(&a)));
            assert_renders(&(a.clone(), b.clone()), &old_tuple(&[a.clone(), b.clone()]));
            assert_renders(&(a.clone(), number, c.clone()), &old_tuple(&[a.clone(), number.to_string(), c.clone()]));

            let nested = vec![list.clone(), other.clone()];
            assert_renders(&nested, &old_vec(&[old_vec(&list), old_vec(&other)]));
            let rows: Vec<(String, String)> = list.iter().cloned().zip(other.iter().cloned()).collect();
            let rendered_rows: Vec<String> = rows.iter().map(|(first, second)| old_tuple(&[first.clone(), second.clone()])).collect();
            assert_renders(&rows, &old_vec(&rendered_rows));
            assert_renders(&(list.clone(), other.clone()), &old_tuple(&[old_vec(&list), old_vec(&other)]));

            let record = Record {
                text: a.clone(),
                list: list.clone(),
                pair: (b.clone(), number),
            };
            let rendered_record = old_struct(&[a.clone(), old_vec(&list), old_tuple(&[b.clone(), number.to_string()])]);
            assert_renders(&record, &rendered_record);
            assert_renders(&vec![record.clone(), record], &old_vec(&[rendered_record.clone(), rendered_record]));

            // The wrappers are new, but have to agree with what they are made of.
            assert_renders(&Counted(list.clone()), &format!("{}\n{}", list.len(), old_vec(&list)));
            let lines: String = list.iter().map(|line| if line.ends_with('\n') { line.clone() } else { format!("{line}\n") }).collect();
            assert_renders(&Lines(list.clone()), &lines);
            assert_renders(&CountedLines(list.clone()), &format!("{}\n{lines}", list.len()));
        }
    }

    #[test]
    fn integers() {
        macro_rules! check {
            ($($int:ty),*) => {
                $(
                    for value in [<$int>::MIN, <$int>::MAX, 0, 1, 9, 10, 99, 100, <$int>::MAX / 10, <$int>::MIN / 10, <$int>::MAX - 1, <$int>::MIN + 1] {
                        assert_renders(&value, &value.to_string());
                    }
                )*
            };
        }
        check!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

        let mut rng = Rng::from_seed(7);
        for _ in 0..20_000 {
            // Every length of number, not just the long ones a uniform draw gives.
            let shift = rng.random_range(0..64_u32);
            let (signed, unsigned) = (rng.random_range::<i64, _>(..) >> shift, rng.random_range::<u64, _>(..) >> shift);
            assert_eq!(signed.to_output(), signed.to_string());
            assert_eq!(unsigned.to_output(), unsigned.to_string());
        }
    }
}

#[cfg(test)]
mod wrapper_tests {
    use crate::{Counted, CountedLines, Lines, ToOutput};

    #[test]
    fn slices_and_arrays_are_written_like_vecs() {
        assert_eq!([1, 2, 3].to_output(), "1 2 3\n");
        assert_eq!([1, 2, 3].as_slice().to_output(), "1 2 3\n");
        assert_eq!([[1, 2], [3, 4]].to_output(), "1 2\n3 4\n");
        assert_eq!(<[i32; 0]>::default().to_output(), "");
        assert_eq!((2, [0.5, 1.5]).to_output(), "2 0.5 1.5\n");
    }

    #[test]
    fn counted() {
        assert_eq!(Counted(vec![5, 7, 9]).to_output(), "3\n5 7 9\n");
        assert_eq!(Counted(Vec::<i32>::new()).to_output(), "0\n");
        assert_eq!(Counted(vec![(1, 2), (3, 4)]).to_output(), "2\n1 2\n3 4\n");
        assert_eq!(Counted(vec![Counted(vec![1]), Counted(vec![2, 3])]).to_output(), "2\n1\n1\n2\n2 3\n");
    }

    #[test]
    fn lines() {
        assert_eq!(Lines(vec![5, 7, 9]).to_output(), "5\n7\n9\n");
        assert_eq!(Lines(Vec::<i32>::new()).to_output(), "");
        assert_eq!(Lines(vec![vec![1, 2], vec![3]]).to_output(), "1 2\n3\n");
        assert_eq!(CountedLines(vec!["ab", "cd"]).to_output(), "2\nab\ncd\n");
        assert_eq!(CountedLines(Vec::<i32>::new()).to_output(), "0\n");
    }

    #[derive(ToOutput)]
    struct Input {
        header: (usize, usize),
        values: Lines<i64>,
        queries: Counted<(usize, usize)>,
    }

    #[test]
    fn wrappers_as_fields() {
        let input = Input {
            header: (2, 1),
            values: Lines(vec![10, 20]),
            queries: Counted(vec![(1, 2)]),
        };
        assert_eq!(input.to_output(), "2 1\n10\n20\n1\n1 2\n");
    }
}
