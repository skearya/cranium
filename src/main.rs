use std::{collections::HashMap, hash::Hash};

use tree_sitter::{Node, Parser};

const SOURCE: &str = include_str!("../input.c");

const SOURCE_BYTES: &[u8] = SOURCE.as_bytes();

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

struct Codegen {
    output: String,
    stack_pointer: usize,
}

impl Codegen {
    fn new() -> Self {
        Self {
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

        self.compound_statement(&body);
    }

    fn compound_statement(&mut self, node: &Node) {
        let mut variables: HashMap<&'static str, usize> = HashMap::new();

        for node in node.named_children(&mut node.walk()) {
            if node.kind() == "declaration" {
                let declarator = node.child_by_field_name("declarator").unwrap();
                let r#type = node.child_by_field_name("type").unwrap();

                let size = match self.src(&r#type) {
                    "char" => 1,
                    _ => panic!(),
                };

                let declarator_inner = declarator.child_by_field_name("declarator").unwrap();

                variables.insert(self.src(&declarator_inner), self.stack_pointer);

                for _ in 0..size {
                    self.output.push('>');
                }

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
                        _ => panic!(),
                    };

                    let declarator_inner = declarator.child_by_field_name("declarator").unwrap();
                    let value = declarator.child_by_field_name("value").unwrap();

                    self.expression(&value, &variables);

                    self.output.push('<');
                    self.stack_pointer -= 1;

                    let var_location = variables[self.src(&declarator_inner)];
                    let var_offset = self.stack_pointer - var_location;

                    self.output.push('[');

                    for _ in 0..var_offset {
                        self.output.push('<');
                    }

                    self.output.push('+');

                    for _ in 0..var_offset {
                        self.output.push('>');
                    }

                    self.output.push('-');

                    self.output.push(']');
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
                _ => self.statement(&node, &variables),
            }
        }
    }

    fn statement(&mut self, node: &Node, variables: &HashMap<&'static str, usize>) {
        match node.kind() {
            "attributed_statement" => todo!(),
            "break_statement" => todo!(),
            "case_statement" => todo!(),
            "compound_statement" => todo!(),
            "continue_statement" => todo!(),
            "do_statement" => todo!(),
            "expression_statement" => {
                let expr = node.child(0).unwrap();

                self.expression(&expr, variables);
            }
            "for_statement" => todo!(),
            "goto_statement" => todo!(),
            "if_statement" => todo!(),
            "labeled_statement" => todo!(),
            "return_statement" => todo!(),
            "seh_leave_statement" => todo!(),
            "seh_try_statement" => todo!(),
            "switch_statement" => todo!(),
            "while_statement" => todo!(),
            x => panic!("{x}"),
        }
    }

    fn expression(&mut self, node: &Node, variables: &HashMap<&'static str, usize>) {
        match node.kind() {
            "alignof_expression" => todo!(),
            "assignment_expression" => todo!(),
            "binary_expression" => {
                fields!(node, left, operator, right);

                self.expression(&left, variables);
                self.expression(&right, variables);

                match self.src(&operator) {
                    "+" => self.output.push_str("<[<+>-]"),
                    "-" => self.output.push_str("<[<->-]"),
                    _ => panic!(),
                }

                self.stack_pointer -= 1;
            }
            "call_expression" => todo!(),
            "cast_expression" => todo!(),
            "char_literal" => todo!(),
            "compound_literal_expression" => todo!(),
            "concatenated_string" => todo!(),
            "conditional_expression" => todo!(),
            "extension_expression" => todo!(),
            "false" => todo!(),
            "field_expression" => todo!(),
            "generic_expression" => todo!(),
            "gnu_asm_expression" => todo!(),
            "identifier" => {
                let var_location = variables[self.src(node)];
                let var_offset = self.stack_pointer - var_location;

                // Copy to two locations

                for _ in 0..var_offset {
                    self.output.push('<');
                }

                self.output.push('[');

                self.output.push('-');

                for _ in 0..var_offset {
                    self.output.push('>');
                }

                self.output.push('+');
                self.output.push('>');
                self.output.push('+');

                for _ in 0..var_offset + 1 {
                    self.output.push('<');
                }

                self.output.push(']');

                // Move destination two into source

                for _ in 0..var_offset + 1 {
                    self.output.push('>');
                }

                self.stack_pointer += 1;

                self.output.push('[');

                self.output.push('-');

                for _ in 0..var_offset + 1 {
                    self.output.push('<');
                }

                self.output.push('+');

                for _ in 0..var_offset + 1 {
                    self.output.push('>');
                }

                self.output.push(']');
            }
            "null" => todo!(),
            "number_literal" => {
                let num = self.src(node).parse::<usize>().unwrap();

                for _ in 0..num {
                    self.output.push('+');
                }

                self.output.push('>');

                self.stack_pointer += 1;
            }
            "offsetof_expression" => todo!(),
            "parenthesized_expression" => todo!(),
            "pointer_expression" => todo!(),
            "sizeof_expression" => todo!(),
            "string_literal" => todo!(),
            "subscript_expression" => todo!(),
            "true" => todo!(),
            "unary_expression" => todo!(),
            "update_expression" => todo!(),
            _ => unreachable!(),
        }
    }

    fn src(&self, node: &Node) -> &'static str {
        node.utf8_text(SOURCE_BYTES)
            .expect("source code should be valid UTF-8")
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

    let mut codegen = Codegen::new();
    codegen.generate(&root);

    dbg!(codegen.output);
}
