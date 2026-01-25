use syrillian::reflection::{function_info, function_infos};

#[syrillian::reflect_fn]
#[allow(unused)]
fn reflected_function(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn function_reflection() {
    let full_name = concat!(module_path!(), "::", "reflected_function");
    let info = function_info(full_name).expect("function should be registered");

    assert_eq!(info.name, "reflected_function");
    assert_eq!(info.module_path, module_path!());
    assert_eq!(info.full_name, full_name);
    assert!(info.signature.contains("fn reflected_function"));

    assert!(
        function_infos()
            .iter()
            .any(|entry| entry.full_name == full_name)
    );
}
