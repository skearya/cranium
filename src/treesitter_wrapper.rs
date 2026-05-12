//! Usability wrapper for tree-sitter's horrendous API (particular
//! to the C language implementation) for use in cranium.

use tree_sitter::Node as TSNode;

trait NodeGroup {
    fn from_node(node: Node) -> Option<Self> where Self: Sized;
}

fn old_get_src<'src>(old: TSNode, src: &'src str) -> &'src str {
    old.utf8_text(src.as_bytes())
        .expect("source code should be valid UTF-8")
}

macro_rules! declare_node_groups {
    (
        $($group_name:ident {
            $($member:ident)|*
            $(|~ $unit_member:ident)*
            $(|* $member_group:ident)*
        }),+ $(,)?
    ) => {
        $(
            pub enum $group_name {
                $($member(Box<$member>),)*
                $($unit_member,)*
                $($member_group(Box<$member_group>),)*
            }

            impl NodeGroup for $group_name {
                fn from_node(node: Node) -> Option<Self> {
                    match node {
                        $(Node::$member(x) => Some(Self::$member(x)),)*
                        $(Node::$unit_member => Some(Self::$unit_member),)*
                        $(x => if let Some(n) = $member_group::from_node(x) {
                            Some(Self::$member_group(Box::new(n)))
                        } else {
                            None
                        },)*
                        #[allow(unreachable_patterns)]
                        _ => None,
                    }
                }
            }
        )+
    };
}

macro_rules! field_stringify {
    ($x:tt) => {
        match stringify!($x) {
            "r#type" => "type",
            x => x,
        }
    };
}

macro_rules! declare_nodes {
    (
        $top_level_type:ident :=
        $(
            $variant_name:ident ($old_name:expr) {
                $(@ $src_name:ident,)?
                $(
                    fields: {
                        $($sv_field_name:ident: $sv_field_type:ident,)*
                        $(* $mv_field_name:ident: $mv_field_type:ident,)*
                        $(? $sv_opt_field_name:ident: $sv_opt_field_type:ident,)*
                        $(?* $mv_opt_field_name:ident: $mv_opt_field_type:ident,)*
                    },
                )?
                $(children: $sv_children_type:ident,)?
                $(* children: $mv_children_type:ident,)?
                $(child: $sv_child_type:ident,)?
                $(* child: $mv_child_type:ident,)?
            },
        )*
        $(
            ~ $unit_variant_name:ident ($unit_old_name:expr),
        )*
    ) => {
        $(
            pub struct $variant_name {
                $(pub $src_name: String)?
                $(
                    $(pub $sv_field_name: Box<$sv_field_type>,)*
                    $(pub $mv_field_name: Box<$mv_field_type>,)*
                    $(pub $sv_opt_field_name: Option<Box<$sv_opt_field_type>>,)*
                    $(pub $mv_opt_field_name: Option<Box<$mv_opt_field_type>>,)*
                )?
                $(pub children: Vec<$sv_children_type>,)?
                $(pub children: Vec<$mv_children_type>,)?
                $(pub child: Box<$sv_child_type>,)?
                $(pub child: Box<$mv_child_type>,)?
            }
        )*

        enum $top_level_type {
            // #[allow(dead_code)]
            $($variant_name(Box<$variant_name>),)*
            $($unit_variant_name,)*
        }

        impl $top_level_type {
            fn from_old(old: TSNode, src: &str) -> Self {
                match old.kind() {
                    $($old_name => {
                        Self::$variant_name(Box::new($variant_name {
                            $($src_name: old_get_src(old, src).to_string(),)?
                            // fields
                            $(
                            $($sv_field_name: Box::new(
                                match Self::from_old(
                                    old.child_by_field_name(field_stringify!($sv_field_name))
                                        .expect("Field should be defined"),
                                    src,
                                ) {
                                    Self::$sv_field_type(x) => *x,
                                    _ => unimplemented!(),
                                }
                            ),)*
                            $($mv_field_name: Box::new(
                                match $mv_field_type::from_node(Self::from_old(
                                    old.child_by_field_name(field_stringify!($mv_field_name))
                                        .expect(concat!("Field should be defined")),
                                    src,
                                )) {
                                    Some(x) => x,
                                    None => panic!("Field node not member of set"),
                                }
                            ),)*
                            $($sv_opt_field_name: old
                                .child_by_field_name(field_stringify!($sv_opt_field_name))
                                .and_then(|old_node| match Self::from_old(old_node, src) {
                                    Self::$sv_opt_field_type(x) => Some(x),
                                    _ => unimplemented!(),
                                }),
                            )*
                            $($mv_opt_field_name: old
                                .child_by_field_name(field_stringify!($mv_opt_field_name))
                                .and_then(|old_node| match $mv_opt_field_type::from_node(
                                    Self::from_old(old_node, src)
                                ) {
                                    Some(x) => Some(Box::new(x)),
                                    None => panic!("Field node not member of set"),
                                }),
                            )*
                            )?
                            // children
                            $(
                            children: old
                                .named_children(&mut old.walk())
                                .map(|n| match Self::from_old(n, src) {
                                    Self::$sv_children_type(x) => *x,
                                    _ => unimplemented!(),
                                }).collect(),
                            )?
                            $(
                            children: old
                                .named_children(&mut old.walk())
                                .map(|n| match $mv_children_type::from_node(
                                    Self::from_old(n, src)
                                ) {
                                    Some(x) => x,
                                    None => panic!("Field node not member of set"),
                                }).collect(),
                            )?
                            $(
                            child: match Self::from_old(
                                old.named_children(&mut old.walk())
                                    .nth(0)
                                    .expect("Node should have a child node"),
                                src,
                            ) {
                                Self::$sv_child_type(x) => x,
                                _ => unimplemented!(),
                            },
                            )?
                            $(
                            child: match $mv_child_type::from_node(
                                Self::from_old(old
                                    .named_children(&mut old.walk())
                                        .nth(0)
                                        .expect("Node should have a child node"),
                                    src,
                                )
                            ) {
                                Some(x) => Box::new(x),
                                None => panic!("Field node not member of set"),
                            },
                            )?
                        }))
                    },)*
                    $($unit_old_name => Self::$unit_variant_name,)*
                    kind => unimplemented!("{}", kind),
                }
            }
        }
    };
}

declare_node_groups! {
    Statement {
        CompoundStatement
        | ExpressionStatement
        | ForStatement
        | IfStatement
        | WhileStatement
    },
    Expression {
        AssignmentExpression
        | BinaryExpression
        | CallExpression
        | CharLiteral
        | Identifier
        | NumberLiteral
        | UpdateExpression
        |~ False
        |~ True
    },
    TypeSpecifier {
        PrimitiveType
    },
    Declarator {
        Identifier
        | InitDeclarator
        | FunctionDeclarator
    },
    BinaryOperator {
        |~ EqualsCheck
        |~ NotEqualsCheck
        |~ Plus
        |~ Minus
    },
    BlockChildren {
        Declaration
        |* Statement
    },
    ForLoopInitializer {
        Declaration
        |* Expression
    },
    CharLiteralChildren {
        Character
        | EscapeSequence
    },
    AssignmentOperator {
        |~ AssignEquals
        |~ PlusEquals
        |~ MinusEquals
    },
    UpdateOperator {
        |~ PlusPlus
        |~ MinusMinus
    },
}

declare_nodes! {
    Node :=
    TranslationUnit ("translation_unit") {
        children: FunctionDefinition,
    },
    CompoundStatement ("compound_statement") {
        * children: BlockChildren,
    },
    FunctionDefinition ("function_definition") {
        fields: {
            body: CompoundStatement,
            * declarator: Declarator,
            * r#type: TypeSpecifier,
        },
    },
    Declaration ("declaration") {
        fields: {
            * declarator: Declarator,
            * r#type: TypeSpecifier,
        },
    },
    Identifier ("identifier") {
        @src,
    },
    PrimitiveType ("primitive_type") {
        @src,
    },
    CharLiteral ("char_literal") {
        * children: CharLiteralChildren,
    },
    NumberLiteral ("number_literal") {
        @src,
    },
    Character ("character") {
        @src,
    },
    EscapeSequence ("escape_sequence") {
        @src,
    },
    ExpressionStatement ("expression_statement") {
        * child: Expression,
    },
    AssignmentExpression ("assignment_expression") {
        fields: {
            left: Identifier,
            * operator: AssignmentOperator,
            * right: Expression,
        },
    },
    BinaryExpression ("binary_expression") {
        fields: {
            * left: Expression,
            * right: Expression,
            * operator: BinaryOperator,
        },
    },
    CallExpression ("call_expression") {
        fields: {
            arguments: ArgumentList,
            * function: Expression,
        },
    },
    ParenthesizedExpression ("parenthesized_expression") {
        * child: Expression,
    },
    ArgumentList ("argument_list") {
        * children: Expression,
    },
    ForStatement ("for_statement") {
        fields: {
            * body: Statement,
            ?* condition: Expression,
            ?* update: Expression,
            ?* initializer: ForLoopInitializer,
        },
    },
    IfStatement ("if_statement") {
        fields: {
            condition: ParenthesizedExpression,
            * consequence: Statement,
            ? alternative: ElseClause,
        },
    },
    WhileStatement ("while_statement") {
        fields: {
            condition: ParenthesizedExpression,
            * body: Statement,
        },
    },
    InitDeclarator ("init_declarator") {
        fields: {
            * declarator: Declarator,
            * value: Expression,
        },
    },
    ElseClause ("else_clause") {
        * child: Statement,
    },
    FunctionDeclarator ("function_declarator") {
        fields: {
            parameters: ParameterList,
            * declarator: Declarator,
        },
    },
    ParameterList ("parameter_list") {
        children: ParameterDeclaration,
    },
    ParameterDeclaration ("parameter_declaration") {
        fields: {
            * declarator: Declarator,
            * r#type: TypeSpecifier,
        },
    },
    UpdateExpression ("update_expression") {
        fields: {
            * argument: Expression,
            * operator: UpdateOperator,
        },
    },
    ~ True ("true"),
    ~ False ("false"),
    ~ AssignEquals ("="),
    ~ EqualsCheck ("=="),
    ~ NotEqualsCheck ("!="),
    ~ Plus ("+"),
    ~ Minus ("-"),
    ~ PlusEquals ("+="),
    ~ MinusEquals ("-="),
    ~ PlusPlus ("++"),
    ~ MinusMinus ("--"),
}

pub fn parse(src: &str) -> TranslationUnit {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .expect("Error loading C parser");
    let tree = parser.parse(src, None).unwrap();
    let root = tree.root_node();

    match Node::from_old(root, src) {
        Node::TranslationUnit(tu) => *tu,
        _ => panic!("Root node was not a translation unit"),
    }
}
