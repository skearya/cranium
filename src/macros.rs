//! Macros commonly used in cranium code generation including
//! helpers to work with the -- frankly -- messy API that
//! tree-sitter provides.

/// Gets named children of a parent `Node`, assuming they exist.
///
/// Sadly branching (using `{...}` syntax) is currently limited
/// to a single use per macro invocation.
///
/// # Usage
/// ```
/// let parent: Node = /* ... */;
///
/// let deep_child = field2!((parent) :: children :: may :: go :: deep);
/// let (ggc, gc2) = field2!((parent) :: child :: {grandchild1 :: ggc, grandchild2});
/// ```
macro_rules! field {
    (($parent:expr) $(:: $field:ident)+) => {
        $parent $(.child_by_field_name(stringify!($field)).unwrap())+
    };
    (($parent:expr) $(:: $field:ident)* :: { $($inner_field:ident $(:: $further_field:ident)*),+ }) => {
        {
            let common = $parent $(.child_by_field_name(stringify!($field)).unwrap())*;

            (
                $(
                    common.child_by_field_name(stringify!($inner_field)).unwrap()
                    $(.child_by_field_name(stringify!($further_field)))*,
                )+
            )
        }
    };
}

/// Yields children nodes of a parent node, where the children
/// may or may not exist.
///
/// Does not have capacity for nested accesses; use `field!()`
/// for that instead.
///
/// # Usage
/// ```
/// let parent: Node = /* ... */;
///
/// let child1: Option<Node> = optional_field!((parent) :: child_name);
/// let (kid1, kid2): (Option<Node>, Option<Node>) = optional_field((parent) :: {kid1_name, kid2_name});
/// ```
macro_rules! optional_field {
    (($parent:expr) :: $field:ident) => {
        $parent.child_by_field_name(stringify!($field))
    };
    (($parent:expr) :: { $($field:ident),+ }) => {
        (
            $($parent.child_by_field_name(stringify!($field)),)+
        )
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
pub(crate) use {field, optional_field};
