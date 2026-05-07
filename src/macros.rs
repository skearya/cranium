//! Macros commonly used in cranium code generation including
//! helpers to work with the -- frankly -- messy API that
//! tree-sitter provides.

/// Declares new `Node`s that are children of `node`given
/// respective names.
/// 
/// ## Example
/// ```
/// let parent = Node::new(/* ... */);
/// fields!(parent: child1, child2);
/// 
/// assert!(matches!(child1, Node));
/// assert!(matches!(child2, Node));
/// ```
macro_rules! fields {
    ($node:ident: $($field:ident),+) => {
        $(let $field = $node
            .child_by_field_name(if stringify!($field) == "r#type" {
                "type"
            } else {
                stringify!($field)
            })
            .unwrap();)*
    };
}

/// Declares new `Node`s that are optional children of `node`
/// given respective names.
/// 
/// ## Example
/// ```
/// let parent = Node::new(/* ... */);
/// optional_fields!(parent: child1, child2);
/// 
/// assert!(matches!(child1, Option<Node>));
/// assert!(matches!(child2, Option<Node>));
/// ```
macro_rules! optional_fields {
    ($node:ident: $($field:ident),+) => {
        $(let $field = $node
            .child_by_field_name(if stringify!($field) == "r#type" {
                "type"
            } else {
                stringify!($field)
            });)*
    };
}

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
pub(crate) use fields;
pub(crate) use optional_fields;
