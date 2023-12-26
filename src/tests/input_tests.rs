#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod generator_tests {
    #[test]
    fn test_input_array() {
        let input_str = "3 1 2 3";
        let mut input = crate::Input::new(input_str);
        let array = input.get_array().unwrap();
        assert_eq!(array, vec![1, 2, 3]);
    }

    #[test]
    fn test_input_array_fail() {
        let input_str = "3 1 2";
        let mut input = crate::Input::new(input_str);
        let array = input.get_array();
        array.unwrap_err();
    }

    #[test]
    fn test_get_int() {
        let input_str = "1";
        let mut input = crate::Input::new(input_str);
        let int = input.get_int().unwrap();
        assert_eq!(int, 1);
    }

    #[test]
    fn test_get_int_fail() {
        let input_str = "e";
        let mut input = crate::Input::new(input_str);
        let int = input.get_int();
        int.unwrap_err();
    }

    #[test]
    fn test_get_int_fail2() {
        let input_str = "";
        let mut input = crate::Input::new(input_str);
        let int = input.get_int();
        int.unwrap_err();
    }

    #[test]
    fn test_get_int_fail3() {
        let input_str = "1.0";
        let mut input = crate::Input::new(input_str);
        let int = input.get_int();
        int.unwrap_err();
    }

    #[test]
    fn test_get_int2() {
        let input_str = "1 2";
        let mut input = crate::Input::new(input_str);
        let int = input.get_int().unwrap();
        assert_eq!(int, 1);
        let int = input.get_int().unwrap();
        assert_eq!(int, 2);
    }

    #[test]
    fn test_get_end() {
        let input_str = "";
        let mut input = crate::Input::new(input_str);
        input.expect_end().unwrap();
    }

    #[test]
    fn test_get_end_fail() {
        let input_str = "1";
        let mut input = crate::Input::new(input_str);
        let end = input.expect_end();
        end.unwrap_err();
    }

    #[test]
    fn test_get_end2() {
        let input_str = "1 2";
        let mut input = crate::Input::new(input_str);
        assert!(input.expect_end().is_err());
        input.get_int().unwrap();
        assert!(input.expect_end().is_err());
        input.get_int().unwrap();
        input.expect_end().unwrap();
    }

    #[test]
    fn test_get_ints() {
        let input_str = "1 2 3";
        let mut input = crate::Input::new(input_str);
        let ints = input.get_ints(3).unwrap();
        assert_eq!(ints, vec![1, 2, 3]);
    }

    #[test]
    fn test_get_ints_fail() {
        let input_str = "1 2";
        let mut input = crate::Input::new(input_str);
        let ints = input.get_ints(3);
        ints.unwrap_err();
    }

    #[test]
    fn test_get_ints2() {
        let input_str = "1 2 3 4 5 6 10 29";
        let mut input = crate::Input::new(input_str);
        let ints = input.get_ints(3).unwrap();
        assert_eq!(ints, vec![1, 2, 3]);
        let ints = input.get_ints(3).unwrap();
        assert_eq!(ints, vec![4, 5, 6]);
        let ints = input.get_ints(0).unwrap();
        assert_eq!(ints, vec![]);
        let ints = input.get_ints(2).unwrap();
        assert_eq!(ints, vec![10, 29]);
    }

    #[test]
    fn test_get_string() {
        let input_str = "abc";
        let mut input = crate::Input::new(input_str);
        let string = input.get_string().unwrap();
        assert_eq!(string, "abc");
    }

    #[test]
    fn test_get_string_fail() {
        let input_str = "";
        let mut input = crate::Input::new(input_str);
        let string = input.get_string();
        string.unwrap_err();
    }

    #[test]
    fn test_get_string2() {
        let input_str = "abc def";
        let mut input = crate::Input::new(input_str);
        let string = input.get_string().unwrap();
        assert_eq!(string, "abc");
        let string = input.get_string().unwrap();
        assert_eq!(string, "def");
    }

    #[test]
    fn test_get_float() {
        let input_str = "1.0";
        let mut input = crate::Input::new(input_str);
        let float = input.get_float().unwrap();
        assert!(f32::abs(float - 1.0) < 1e-6);
    }

    #[test]
    fn test_get_float_fail() {
        let input_str = "e";
        let mut input = crate::Input::new(input_str);
        let float = input.get_float();
        float.unwrap_err();
    }

    #[test]
    fn test_get_float2() {
        let input_str = "1.0 2.0";
        let mut input = crate::Input::new(input_str);
        let float = input.get_float().unwrap();
        assert!(f32::abs(float - 1.0) < 1e-6);
        let float = input.get_float().unwrap();
        assert!(f32::abs(float - 2.0) < 1e-6);
    }
}
