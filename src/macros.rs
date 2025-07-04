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
