//! Macros commonly used in cranium code generation.

/// Pushes a loop to the generated BF with everything
/// inside `block` being executed between the loop delimiters.
macro_rules! bf_loop {
    ($codegen:ident, $block:block) => {
        $codegen.push('[');
        $block
        $codegen.push(']');
    };
}

pub(crate) use bf_loop;
