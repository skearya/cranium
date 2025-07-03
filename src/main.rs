use std::collections::HashMap;

use tree_sitter::{Node, Parser};

const SOURCE: &str = include_str!("../input.c");

// TODO: Add optional support
macro_rules! fields {
    ($node:ident, $field:ident) => {
        let $field = $node.child_by_field_name(stringify!($field)).unwrap();
    };

    ($node:ident, $field:ident, $($fields:ident),+) => {
        fields!($node, $field);
        fields!($node, $($fields),+)
    };
}

macro_rules! bf_loop {
    ($codegen:ident, $block:block) => {
        $codegen.push('[');
        $block
        $codegen.push(']');
    };
}

struct Codegen<'src> {
    src: &'src str,
    output: String,
    stack_pointer: usize,
}

struct Environment<'src, 'a> {
    parent: Option<&'a Environment<'src, 'a>>,
    variables: HashMap<&'src str, usize>,
}

impl<'src> Environment<'src, '_> {
    fn lookup(&self, name: &'src str) -> Option<usize> {
        match self.variables.get(name) {
            Some(location) => Some(*location),
            None => match self.parent {
                Some(parent) => parent.lookup(name),
                None => None,
            },
        }
    }
}

impl<'src> Codegen<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            src,
            output: String::new(),
            stack_pointer: 0,
        }
    }

    fn generate(&mut self, root: &Node) {
        for node in root.named_children(&mut root.walk()) {
            match node.kind() {
                "attributed_statement" => todo!(),
                "break_statement" => todo!(),
                "case_statement" => todo!(),
                "compound_statement" => todo!(),
                "continue_statement" => todo!(),
                "declaration" => todo!(),
                "do_statement" => todo!(),
                "expression_statement" => todo!(),
                "for_statement" => todo!(),
                "function_definition" => {
                    fields!(node, declarator);

                    // Access identifier of declarator named "declarator".
                    fields!(declarator, declarator);

                    if self.src(&declarator) == "main" {
                        self.main(&node);
                    }
                }
                "goto_statement" => todo!(),
                "if_statement" => todo!(),
                "labeled_statement" => todo!(),
                "linkage_specification" => todo!(),
                "preproc_call" => todo!(),
                "preproc_def" => todo!(),
                "preproc_function_def" => todo!(),
                "preproc_if" => todo!(),
                "preproc_ifdef" => todo!(),
                "preproc_include" => todo!(),
                "return_statement" => todo!(),
                "switch_statement" => todo!(),
                "type_definition" => todo!(),
                "type_specifier" => todo!(),
                "while_statement" => todo!(),
                _ => unreachable!(),
            }
        }
    }

    fn main(&mut self, node: &Node) {
        fields!(node, body);

        self.compound_statement(&body, None);
    }

    fn compound_statement(&mut self, node: &Node, parent: Option<&Environment>) {
        let mut env = Environment {
            parent,
            variables: HashMap::new(),
        };

        let stack_base = self.stack_pointer;

        for node in node.named_children(&mut node.walk()) {
            if node.kind() == "declaration" {
                let declarator = node.child_by_field_name("declarator").unwrap();
                let r#type = node.child_by_field_name("type").unwrap();

                let size = match self.src(&r#type) {
                    "char" => 1,
                    "bool" => 1,
                    _ => panic!(),
                };

                let declarator_inner = declarator.child_by_field_name("declarator").unwrap();

                env.variables
                    .insert(self.src(&declarator_inner), self.stack_pointer);

                self.push_n(size, '>');
                self.stack_pointer += size;
            }
        }

        for node in node.named_children(&mut node.walk()) {
            match node.kind() {
                "declaration" => {
                    let declarator = node.child_by_field_name("declarator").unwrap();
                    let r#type = node.child_by_field_name("type").unwrap();

                    match self.src(&r#type) {
                        "char" => {}
                        "bool" => {}
                        _ => panic!(),
                    }

                    let declarator_inner = declarator.child_by_field_name("declarator").unwrap();
                    let value = declarator.child_by_field_name("value").unwrap();

                    self.expression(&value, &env);

                    self.push('<');
                    self.stack_pointer -= 1;

                    let var_location = env.variables[self.src(&declarator_inner)];
                    let var_offset = self.stack_pointer - var_location;

                    bf_loop!(self, {
                        self.push_n(var_offset, '<');
                        self.push('+');
                        self.push_n(var_offset, '>');
                        self.push('-');
                    });
                }
                "function_definition" => todo!(),
                "linkage_specification" => todo!(),
                "preproc_call" => todo!(),
                "preproc_def" => todo!(),
                "preproc_function_def" => todo!(),
                "preproc_if" => todo!(),
                "preproc_ifdef" => todo!(),
                "preproc_include" => todo!(),
                "type_definition" => todo!(),
                "type_specifier" => todo!(),
                _ => self.statement(&node, &env),
            }
        }

        let stack_size = self.stack_pointer - stack_base;

        for _ in 0..stack_size {
            self.push('<');
            self.push_str("[-]");
        }

        self.stack_pointer -= stack_size;
    }

    fn statement(&mut self, node: &Node, env: &Environment) {
        match node.kind() {
            "attributed_statement" => todo!(),
            "break_statement" => todo!(),
            "case_statement" => todo!(),
            "compound_statement" => self.compound_statement(node, Some(env)),
            "continue_statement" => todo!(),
            "do_statement" => todo!(),
            "expression_statement" => {
                let expr = node.child(0).unwrap();

                self.expression(&expr, env);
            }
            "for_statement" => todo!(),
            "goto_statement" => todo!(),
            "if_statement" => {
                fields!(node, condition, consequence);

                self.expression(&condition.named_child(0).unwrap(), env);

                self.push('<');

                bf_loop!(self, {
                    self.push_str("[-]");
                    self.stack_pointer -= 1;

                    self.statement(&consequence, env);
                });
            }
            "labeled_statement" => todo!(),
            "return_statement" => todo!(),
            "seh_leave_statement" => todo!(),
            "seh_try_statement" => todo!(),
            "switch_statement" => todo!(),
            "while_statement" => todo!(),
            x => panic!("{x}"),
        }
    }

    fn expression(&mut self, node: &Node, env: &Environment) {
        match node.kind() {
            "alignof_expression" => todo!(),
            "assignment_expression" => {
                fields!(node, left, right, operator);

                match left.kind() {
                    "call_expression" => todo!(),
                    "field_expression" => todo!(),
                    "identifier" => {}
                    "parenthesized_expression" => todo!(),
                    "pointer_expression" => todo!(),
                    "subscript_expression" => todo!(),
                    _ => unreachable!(),
                }

                match self.src(&operator) {
                    "%=" => todo!(),
                    "&=" => todo!(),
                    "*=" => todo!(),
                    "+=" => todo!(),
                    "-=" => todo!(),
                    "/=" => todo!(),
                    "<<=" => todo!(),
                    "=" => {}
                    ">>=" => todo!(),
                    "^=" => todo!(),
                    "|=" => todo!(),
                    _ => unreachable!(),
                };

                self.expression(&right, env);

                self.push('<');
                self.stack_pointer -= 1;

                let var_location = env
                    .lookup(self.src(&left))
                    .expect("variable should've been found");
                let var_offset = self.stack_pointer - var_location;

                // Clear memory

                self.push_n(var_offset, '<');
                self.push_str("[-]");
                self.push_n(var_offset, '>');

                bf_loop!(self, {
                    self.push_n(var_offset, '<');
                    self.push('+');
                    self.push_n(var_offset, '>');
                    self.push('-');
                });
            }
            "binary_expression" => {
                fields!(node, left, operator, right);

                self.expression(&left, env);
                self.expression(&right, env);

                match self.src(&operator) {
                    "+" => self.push_str("<[<+>-]"),
                    "-" => self.push_str("<[<->-]"),
                    _ => todo!(),
                }

                self.stack_pointer -= 1;
            }
            "call_expression" => {
                fields!(node, function, arguments);

                // TODO: Functions can be any expression
                match self.src(&function) {
                    "putc" => {}
                    _ => panic!(),
                };

                self.argument_list(&arguments, env);

                self.push_str("<.[-]");

                self.stack_pointer -= arguments.named_child_count()
            }
            "cast_expression" => todo!(),
            "char_literal" => {
                if node.named_child_count() != 1 {
                    panic!("expected one character in char literal")
                };

                let child = node.named_child(0).unwrap();

                let char = match child.kind() {
                    "character" => self.src(&child).chars().next().unwrap(),
                    "escape_sequence" => match self.src(&child) {
                        r#"\'"# => '\'',
                        r#"\""# => '\"',
                        r#"\?"# => '?',
                        r#"\\"# => '\\',
                        r#"\a"# => '\x07',
                        r#"\b"# => '\x08',
                        r#"\f"# => '\x0c',
                        r#"\n"# => '\n',
                        r#"\r"# => '\r',
                        r#"\t"# => '\t',
                        r#"\v"# => '\x0b',
                        esc if esc.starts_with(r#"\x"#) => todo!(),
                        esc if esc.starts_with(r#"\u"#) => todo!(),
                        esc if esc.starts_with(r#"\U"#) => todo!(),
                        esc if esc.starts_with(r#"\"#) => todo!(),
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                };

                self.push_n(char as usize, '+');
                self.push('>');

                self.stack_pointer += 1;
            }
            "compound_literal_expression" => todo!(),
            "concatenated_string" => todo!(),
            "conditional_expression" => todo!(),
            "extension_expression" => todo!(),
            "false" => {
                self.push('>');

                self.stack_pointer += 1;
            }
            "field_expression" => todo!(),
            "generic_expression" => todo!(),
            "gnu_asm_expression" => todo!(),
            "identifier" => {
                let var_location = env
                    .lookup(self.src(node))
                    .expect("variable should've been found");
                let var_offset = self.stack_pointer - var_location;

                // Copy to two locations

                self.push_n(var_offset, '<');

                bf_loop!(self, {
                    self.push('-');
                    self.push_n(var_offset, '>');
                    self.push('+');
                    self.push('>');
                    self.push('+');
                    self.push_n(var_offset + 1, '<');
                });

                // Move destination two back into source

                self.push_n(var_offset + 1, '>');

                bf_loop!(self, {
                    self.push('-');
                    self.push_n(var_offset + 1, '<');
                    self.push('+');
                    self.push_n(var_offset + 1, '>');
                });

                self.stack_pointer += 1;
            }
            "null" => todo!(),
            "number_literal" => {
                let num = self.src(node).parse::<usize>().unwrap();

                self.push_n(num, '+');
                self.push('>');

                self.stack_pointer += 1;
            }
            "offsetof_expression" => todo!(),
            "parenthesized_expression" => todo!(),
            "pointer_expression" => todo!(),
            "sizeof_expression" => todo!(),
            "string_literal" => todo!(),
            "subscript_expression" => todo!(),
            "true" => {
                self.push('+');
                self.push('>');

                self.stack_pointer += 1;
            }
            "unary_expression" => todo!(),
            "update_expression" => todo!(),
            _ => unreachable!(),
        }
    }

    fn argument_list(&mut self, node: &Node, env: &Environment) {
        for argument in node.named_children(&mut node.walk()) {
            // TODO: Do not assume the argument is an expression
            self.expression(&argument, env);
        }
    }

    fn src(&self, node: &Node) -> &'src str {
        node.utf8_text(self.src.as_bytes())
            .expect("source code should be valid UTF-8")
    }

    fn push(&mut self, c: char) {
        debug_assert!(matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']'));

        self.output.push(c);
    }

    fn push_n(&mut self, n: usize, c: char) {
        for _ in 0..n {
            self.push(c);
        }
    }

    fn push_str(&mut self, s: &str) {
        self.output.push_str(s);
    }
}

fn main() {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .expect("Error loading C parser");

    let tree = parser.parse(SOURCE, None).unwrap();
    let root = tree.root_node();

    dbg!(root.to_sexp());

    let mut codegen = Codegen::new(SOURCE);
    codegen.generate(&root);

    dbg!(codegen.output);
}
