//! Usability wrapper for tree-sitter's horrendous API (particular
//! to the C language implementation) for use in cranium.

use tree_sitter::Node as TSNode;

/// Several `Node` variants grouped together.
trait NodeGroup {
    /// Converting a general `Node` into a member of `Self`.
    /// If the conversion could not be made (i.e., `node`'s
    /// variant was not in `Self`) this function returns `None`.
    fn from_node(node: Node) -> Option<Self> where Self: Sized;

    /// Returns whether `node` would fit in `Self`. Uses very
    /// similar logic as `from_node` but does not consume
    /// `node`.
    fn matches(node: &Node) -> bool;
}

/// Takes a treesitter node (`old`) and the source
/// file's code (`src`) and returns the slice associated
/// with `old`.
fn old_get_src<'src>(old: TSNode, src: &'src str) -> &'src str {
    old.utf8_text(src.as_bytes())
        .expect("source code should be valid UTF-8")
}

/// Declares several node group enums, which all implement the
/// `NodeGroup` trait.
/// 
/// This is one of the two macros used in the tree-sitter wrapper
/// to convert tree-sitter nodes (`TSNode`s) into the fully-typed
/// system that cranium uses (internally, `Node`s). The other of
/// which is `declare_nodes!`.
/// 
/// # Usage
/// 
/// You define a node group with its name (conventionally in
/// PascalCase) followed by curly braces where you provide a
/// comma-separated list of the group's variants. The variants come
/// in 3 forms: (1) top-level nodes, (2) data-less (aka unit) nodes,
/// and (3) other node groups. Each variant is clarified prior to
/// entry with a prepended token (or the absence of one).
/// 
/// 1. **Top-level nodes** (no prefix): Regular nodes which have
/// associated data that just knowing the variant isn't enough
/// information for (e.g., `Identifier`s, because you still need
/// the text they refer to).
/// 
/// 2. **Data-less (Unit) nodes** (`~` prefix): Just like (1) but
/// there is no further information needed to interpret them than
/// the variant (e.g., `Plus`s). The associated `Node` variant must
/// also be data-less.
/// 
/// 3. **Other node groups** (`*` prefix): Other node groups defined
/// in this macro invocation (e.g., `Declarator`s).
/// 
/// Variants must be declared in this order (just a limitation of
/// `macro_rules!` macros).
/// 
/// This gets converted into a `pub enum` with variants named exactly
/// as they're provided, and the data-ful ones each come with a `Box`
/// to the type of the same name as the variant. For this reason,
/// **every variant name must be identical to its associated type**.
/// This was chosen for simplicity as well as unambiguity, although
/// perhaps at the expense of consiceness.
/// 
/// ## Example
/// 
/// ```
/// declare_node_groups! {
///     GroupName {
///         MemberNode1,
///         MemberNode2,
///         ~ NoDataMemberNode,
///         * NodeGroup,
///     },
///     // ...
/// }
/// ```
/// converts to...
/// ```
/// enum GroupName {
///     MemberNode1(Box<MemberNode1>),
///     MemberNode2(Box<MemberNode2>),
///     NoDataMemberNode, // Always matches to `Node::NoDataMemberNode`
///     OtherGroup(Box<OtherGroup>),
/// }
/// 
/// impl NodeGroup for GroupName {
///     fn from_node(node: Node) -> Option<Self> {
///         match node {
///             Node::MemberNode1(n) => Some(Self::MemberNode1(n)),
///             Node::MemberNode2(n) => Some(Self::MemberNode1(n)),
///             Node::NoDataMemberNode => Some(Self::NoDataMemberNode),
///             n if OtherGroup::matches(&n) => Some(Self::from_node(n).unwrap()),
///             _ => None,
///         }
///     }
/// 
///     fn matches(node: &Node) -> bool {
///         match node {
///             Node::MemberNode1(_) => true,
///             Node::MemberNode2(_) => true,
///             Node::NoDataMemberNode => true,
///             n if OtherGroup::matches(n) => true,
///             _ => false,
///         }
///     }
/// }
/// 
/// /// ...
/// ```
macro_rules! declare_node_groups {
    (
        $($group_name:ident {
            $($member:ident,)*
            $(~ $unit_member:ident,)*
            $(* $member_group:ident,)*
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
                        $(Node::$member(n) => Some(Self::$member(n)),)*
                        $(Node::$unit_member => Some(Self::$unit_member),)*
                        $(n if $member_group::matches(&n) => Some(
                            Self::$member_group(
                                Box::new($member_group::from_node(n).unwrap())
                            )
                        ),)*
                        #[allow(unreachable_patterns)]
                        _ => None,
                    }
                }

                fn matches(node: &Node) -> bool {
                    match node {
                        $(Node::$member(_) => true,)*
                        $(Node::$unit_member => true,)*
                        $(x if $member_group::matches(x) => true,)*
                        _ => false,
                    }
                }
            }
        )+
    };
}

/// Glorified `stringify!` except if the passed token
/// stream is `r#type`, it converts it to `"type"`.
macro_rules! field_stringify {
    ($x:tt) => {
        match stringify!($x) {
            "r#type" => "type",
            x => x,
        }
    };
}

/// Declares all the different node variants that the language
/// is meant to support.
/// 
/// This is one of the two macros used in the tree-sitter wrapper
/// to convert tree-sitter nodes (`TSNode`s) into the fully-typed
/// system that cranium uses (internally, `Node`s). The other of
/// which is `declare_node_groups!`.
/// 
/// # Usage
/// 
/// At the top of the macro invocation you specify what the overall node type will be. This should be `Node` but for a semblance of hygeine it's allowed to be specified here.
/// 
/// You define a node variant with its internal name (conventionally in PascalCase) followed by its tree-sitter string name enclosed in parentheses, then followed by curly braces that contain information about the data associated with the node. The three data "fields" that you may provide are (1) whether - and by what moniker - to associate the node with its source code, (2) the actual fields that the node will have, and (3) the child(ren) that the node will have.
/// 
/// 1. **Sourcecode endpoint** (`@` prefix): If defined, specifies what identifier the sourcecode associated with the node should be. This is generally `src` but the option is available for something else.
/// 
/// 2. **Fields**: If defined, constitute the fields - required or optional - that the node possesses. They are specified by `field:` followed by a curly-brace-enclosed comma-separated list of fields, which come in 4 separate forms:
/// 
///     a. **Single-variant required field** (no prefix): Field where there is exactly one node variant that it may be (e.g., `FunctionDefinition`'s `body` field must be a `CompoundStatement` and nothing else).
/// 
///     b. **Multi-variant required field** (`*` prefix): Field where there are several variants that would satisfy the field, which must have a corresponding node group (see `declare_node_groups!`) (e.g., `Declaration`'s `declarator` field may be any node variant contained in `Declarator`).
/// 
///     c. **Single-variant optional field** (`?` prefix): Just like its required counterpart, but it may also not be fulfilled at all.
/// 
///     c. **Multi-variant optional field** (`?*` prefix): Just like its required counterpart, but it may also not be fulfilled at all.
/// 
/// 3. **Children**: If defined, describe potential children of the node and the variants they may be of. Unlike fields, children do not associate with any name in relation to the parent node. They may also exist in arbitrary numbers in some cases, which is very useful. The child(ren) of a node may be set in one of four configurations (future iterations may have more):
/// 
///     a. **Single-variant multi-count children** (`children:` prefix): Children who have only a single variant that they may be and there may also be several instances of them.
/// 
///     b. **Multi-variant multi-count children** (`* children:` prefix): Just like their single-variant counterparts but they may be of the form of any member of a specified group (see `declare_node_groups!`).
/// 
///     c. **Single-variant single-count children** (`child:` prefix): Just like their multi-count counterparts but there may only exist one child of the node.
/// 
///     d. **Multi-variant single-count children** (`* child:` prefix): Just like their single-variant counterparts but the node may be in the form of any member of a specified group (see `declare_node_groups!`).
/// 
/// After all those nodes have been specified, there exists a space for data-less (unit) node variants. They carry with them no semantic information other than their variant and how tree-sitter expressed them. They are specified like all the nodes prior, but before them comes a `~` and there are no curly braces (e.g., `~ Variant ("variant"),`).
/// 
/// The top-level type (`Node` in cranium) is an enum over all the supplied variants, and also implements the `from_old` function, which takes in a tree-sitter node (`TSNode`) and outputs the corresponding cranium `Node`.
/// 
/// ## Example
/// 
/// ```
/// declare_nodes! {
///     Node :=
///     Variant1 ("variant_1") {
///         fields: {
///             field1: Variant2,
///             * multi_variant_field: Group1,
///             ? optional_field: Variant3,
///             ?* multi_variant_optional_field: Group2,
///         }
///         // can only choose one child configuration for the example
///         * children: Group3,
///     }
///     // ...
/// }
/// ```
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
        CompoundStatement,
        ExpressionStatement,
        ForStatement,
        IfStatement,
        WhileStatement,
    },
    Expression {
        AssignmentExpression,
        BinaryExpression,
        CallExpression,
        CharLiteral,
        Identifier,
        NumberLiteral,
        UpdateExpression,
        ~ False,
        ~ True,
    },
    TypeSpecifier {
        PrimitiveType,
    },
    Declarator {
        Identifier,
        InitDeclarator,
        FunctionDeclarator,
    },
    BinaryOperator {
        ~ EqualsCheck,
        ~ NotEqualsCheck,
        ~ Plus,
        ~ Minus,
    },
    BlockChildren {
        Declaration,
        * Statement,
    },
    ForLoopInitializer {
        Declaration,
        * Expression,
    },
    CharLiteralChildren {
        Character,
        EscapeSequence,
    },
    AssignmentOperator {
        ~ AssignEquals,
        ~ PlusEquals,
        ~ MinusEquals,
    },
    UpdateOperator {
        ~ PlusPlus,
        ~ MinusMinus,
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

/// Parses C file (`src`) and returns the top-level
/// node of the file, a `TranslationUnit`.
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
