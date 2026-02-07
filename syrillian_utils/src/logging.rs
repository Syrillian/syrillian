#[macro_export]
macro_rules! debug_panic {
    () => ( if cfg!(debug_assertions) { panic!($($arg)*); } );
    ($($arg:tt)*) => ( if cfg!(debug_assertions) { panic!($($arg)*); } else { ::syrillian_utils::tracing::error!($($arg)*); } );
}
