//! Code generation logic for cranium.

use std::collections::HashMap;

use tree_sitter::{Node, Parser};

use crate::macros::{bf_loop, fields, optional_fields};

/// Stateful type keeping track of the C to BF code
/// generation.
pub struct Codegen<'src> {
    /// Source C code.
    src: &'src str,
    /// Tracked location of the stack pointer. In other
    /// words, the index that the head is currently at.
    stack_pointer: usize,
    /// Output BF code.
    output: String,
}

/// Information about a scope's variables, regarding
/// where they reside and where the locals begin.
/// 
/// Allows redefining variables inside inner scopes
/// while leaving their value in outer scopes unchanged.
struct Environment<'src, 'a> {
    /// The parent environment.
    parent: Option<&'a Environment<'src, 'a>>,
    /// Maps variable name to absolute location.
    variables: HashMap<&'src str, usize>,
    /// Absolute location of the beginning of
    /// the local varaibles for the current scope.
    stack_base: usize,
}

impl<'src> Environment<'src, '_> {
    /// Returns absolute location of `name` variable.
    fn lookup(&self, name: &str) -> Option<usize> {
        match self.variables.get(name) {
            Some(location) => Some(*location),
            None => match self.parent {
                Some(parent) => parent.lookup(name),
                None => None,
            },
        }
    }
}

/// Returns whether a `Node::kind()` is a statement
fn is_statement(kind: &str) -> bool {
    matches!(
        kind,
        "attributed_statement"
            | "break_statement"
            | "case_statement"
            | "compound_statement"
            | "continue_statement"
            | "do_statement"
            | "expression_statement"
            | "for_statement"
            | "goto_statement"
            | "if_statement"
            | "labeled_statement"
            | "return_statement"
            | "seh_leave_statement"
            | "seh_try_statement"
            | "switch_statement"
            | "while_statement"
    )
}

/// Returns whether a `Node::kind()` is an expression.
fn is_expression(kind: &str) -> bool {
    matches!(
        kind,
        "alignof_expression"
            | "assignment_expression"
            | "binary_expression"
            | "call_expression"
            | "cast_expression"
            | "char_literal"
            | "compound_literal_expression"
            | "concatenated_string"
            | "conditional_expression"
            | "extension_expression"
            | "false"
            | "field_expression"
            | "generic_expression"
            | "gnu_asm_expression"
            | "identifier"
            | "null"
            | "number_literal"
            | "offsetof_expression"
            | "parenthesized_expression"
            | "pointer_expression"
            | "sizeof_expression"
            | "string_literal"
            | "subscript_expression"
            | "true"
            | "unary_expression"
            | "update_expression"
    )
}

impl<'src> Codegen<'src> {
    /// Initializes a `Codegen` object given C source code.
    pub fn new(src: &'src str) -> Self {
        Self {
            src,
            stack_pointer: 0,
            output: String::new(),
        }
    }

    /// Returns a string slice to the exact source code
    /// corresponding to a `Node`.
    fn src(&self, node: Node) -> &'src str {
        node.utf8_text(self.src.as_bytes())
            .expect("source code should be valid UTF-8")
    }

    /// Pushes `c` to the generated BF code.
    fn push(&mut self, c: char) {
        debug_assert!(matches!(
            c,
            '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']' | '@'
        ));

        self.output.push(c);
    }

    /// Pushes `n` instances of `c` to the generated BF code.
    fn push_n(&mut self, n: usize, c: char) {
        for _ in 0..n {
            self.push(c);
        }
    }

    /// Pushes `s` to the generated BF code.
    fn push_str(&mut self, s: &str) {
        debug_assert!(
            s.chars()
                .all(|c| matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']' | '@'))
        );

        self.output.push_str(s);
    }

    /// Pushes `n` instances of `s` to the generated BF code.
    fn push_n_str(&mut self, n: usize, s: &str) {
        for _ in 0..n {
            self.push_str(s);
        }
    }

    /// Clears the all contents of `env`'s local variables,
    /// resetting the codegen's stack pointer to base as well.
    fn clear_environment(&mut self, env: Environment) {
        let locals_size = self.stack_pointer - env.stack_base;
        
        self.push_n_str(locals_size, "<[-]");

        self.stack_pointer = env.stack_base;
    }

    /// Adds `declaration` to the environment, reserving space for
    /// it and adding it to `env.variables`.
    /// 
    /// Assumes the stack pointer is at the appropriate location
    /// to insert the variable.
    fn add_variable(&mut self, env: &mut Environment<'src, '_>, declaration: Node) {
        debug_assert_eq!(declaration.kind(), "declaration");

        fields!(declaration: declarator, r#type);

        // Sizes over 1 coming soon...
        let size: usize = match self.src(r#type) {
            "char" | "bool" => 1,
            _ => unimplemented!(),
        };

        let name = match declarator.kind() {
            "array_declarator" => unimplemented!(),
            "attributed_declarator" => unimplemented!(),
            "function_declarator" => unimplemented!(),
            "gnu_asm_expression" => unimplemented!(),
            "identifier" => self.src(declarator),
            "init_declarator" => {
                fields!(declarator: declarator /* , value */);
                assert_eq!(declarator.kind(), "identifier", "Only supporting basic declarations at present");

                self.src(declarator)
            },
            "ms_call_modifier" => unimplemented!(),
            "parenthesized_declarator" => unimplemented!(),
            "pointer_declarator" => unimplemented!(),
            _ => unreachable!(),
        };

        if env.lookup(name).is_some() {
            panic!("Colliding variable declaration");
        }

        env.variables.insert(name, self.stack_pointer);

        self.push_n(size, '>');
        self.stack_pointer += size;
    }

    /// Top-level call to compile the C file to BF.
    pub fn generate(mut self) -> String {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .expect("Error loading C parser");
        let tree = parser.parse(self.src, None).unwrap();
        let root = tree.root_node();
        
        assert_eq!(root.kind(), "translation_unit");

        self.translation_unit(root);

        self.output
    }

    /// Generate code for a `translation_unit` node.
    /// 
    /// For the purposes of this project this refers to
    /// a parsed C file.
    fn translation_unit(&mut self, root: Node) {
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
                    fields!(node: declarator);

                    // Access identifier of declarator named "declarator".
                    fields!(declarator: declarator);

                    if self.src(declarator) == "main" {
                        self.main(node);
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
                "preproc_include" => {
                    println!("imports aren't supported yet");
                }
                "return_statement" => todo!(),
                "switch_statement" => todo!(),
                "type_definition" => todo!(),
                // haha this is actually several kinds!
                // if only the tree sitter api wasnt
                // so incredibly sloppy...
                "type_specifier" => todo!(),
                "while_statement" => todo!(),
                _ => unreachable!(),
            }
        }
    }

    /// Generate code for the `main` function, which is
    /// where program execution begins.
    fn main(&mut self, node: Node) {
        fields!(node: body);

        self.compound_statement(body, None);
    }

    /// This generates code for a scoping block (known internally
    /// as a `compound_statement`). Creates a new environment for
    /// the local variables declared here.
    fn compound_statement(&mut self, node: Node, parent: Option<&Environment<'src, '_>>) {
        let mut env = Environment {
            parent,
            variables: HashMap::new(),
            stack_base: self.stack_pointer,
        };

        for declaration in node
            .named_children(&mut node.walk())
            .filter(|node| node.kind() == "declaration")
        {
            self.add_variable(&mut env, declaration);
        }

        for node in node.named_children(&mut node.walk()) {
            match node.kind() {
                "declaration" => self.declaration(node, &env),
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
                kind if is_statement(kind) => self.statement(node, &env),
                _ => unreachable!(),
            }
        }

        self.clear_environment(env);
    }

    /// Generates code for a variable declaration, assuming
    /// the environment already has an assigned location for it.
    fn declaration(&mut self, node: Node, env: &Environment) {
        fields!(node: declarator, r#type);

        match self.src(r#type) {
            "char" | "bool" => {},
            _ => todo!(),
        }

        fields!(declarator: value);
        fields!(declarator: declarator);

        self.expression(value, env);

        self.push('<');
        self.stack_pointer -= 1;

        let var_location = env.variables[self.src(declarator)];
        let var_offset = self.stack_pointer - var_location;

        bf_loop!(self, {
            self.push_n(var_offset, '<');
            self.push('+');
            self.push_n(var_offset, '>');
            self.push('-');
        });
    }

    /// Generates code for any statement.
    fn statement(&mut self, node: Node, env: &Environment<'src, '_>) {
        match node.kind() {
            "attributed_statement" => todo!(),
            "break_statement" => todo!(),
            "case_statement" => todo!(),
            "compound_statement" => self.compound_statement(node, Some(env)),
            "continue_statement" => todo!(),
            "do_statement" => todo!(),
            "expression_statement" => {
                let child = node.child(0).unwrap();

                match child.kind() {
                    "comma_expression" => todo!(),
                    kind if is_expression(kind) => self.expression(child, env),
                    _ => unreachable!(),
                }
            }
            "for_statement" => {
                fields!(node: body);

                // this is semantically equivalent to `fields`?
                optional_fields!(node: initializer, condition, update);
                let initializer = initializer.unwrap();
                let condition = condition.unwrap();
                let update = update.unwrap();


                match initializer.kind() {
                    "comma_expression" => todo!(),
                    "declaration" => {}
                    kind if is_expression(kind) => todo!(),
                    _ => unreachable!(),
                }

                match condition.kind() {
                    "comma_expression" => todo!(),
                    kind if is_expression(kind) => {}
                    _ => unreachable!(),
                }

                match update.kind() {
                    "comma_expression" => todo!(),
                    kind if is_expression(kind) => {}
                    _ => unreachable!(),
                }

                let mut outer_env = Environment {
                    parent: Some(env),
                    variables: HashMap::new(),
                    stack_base: self.stack_pointer,
                };

                self.add_variable(&mut outer_env, initializer);

                self.expression(condition, &outer_env);
                self.push('<');
                self.stack_pointer -= 1;

                bf_loop!(self, {
                    self.push_str("[-]");

                    self.statement(body, &outer_env);

                    self.expression(update, &outer_env);

                    self.expression(condition, &outer_env);
                    self.push('<');
                    self.stack_pointer -= 1;
                });
            }
            "goto_statement" => todo!(),
            "if_statement" => {
                fields!(node: condition, consequence);
                optional_fields!(node: alternative);

                if let Some(alternative) = alternative {
                    // Init flag to 1
                    self.push('+');
                    self.push('>');
                    self.stack_pointer += 1;

                    self.parenthesized_expression(condition, env);

                    self.push('<');
                    self.stack_pointer -= 1;

                    // Not actual loop. Resets flag if cond != 0
                    bf_loop!(self, {
                        self.push_str("<->");
                        self.push_str("[-]");

                        self.statement(consequence, env);
                    });

                    // Cond space guaranteed to be zero
                    self.push('<');
                    self.stack_pointer -= 1;

                    // Another not-a-loop. Executes on flag
                    bf_loop!(self, {
                        self.push('-');

                        self.statement(alternative.named_child(0).unwrap(), env);
                    });
                } else {
                    self.parenthesized_expression(condition, env);

                    self.push('<');
                    self.stack_pointer -= 1;

                    bf_loop!(self, {
                        self.push_str("[-]");

                        self.statement(consequence, env);
                    });
                }
            }
            "labeled_statement" => todo!(),
            "return_statement" => todo!(),
            "seh_leave_statement" => todo!(),
            "seh_try_statement" => todo!(),
            "switch_statement" => todo!(),
            "while_statement" => {
                fields!(node: body, condition);

                self.parenthesized_expression(condition, env);
                self.push('<');
                self.stack_pointer -= 1;

                bf_loop!(self, {
                    self.push_str("[-]");

                    self.statement(body, env);

                    self.parenthesized_expression(condition, env);
                    self.push('<');
                    self.stack_pointer -= 1;
                });
            }
            _ => unreachable!(),
        }
    }

    /// Evaluates any expression and leaves its value on stack.
    fn expression(&mut self, node: Node, env: &Environment) {
        match node.kind() {
            "alignof_expression" => todo!(),

            // TODO: make it actually an expression (return rvalue)
            "assignment_expression" => {
                fields!(node: left, right, operator);

                match left.kind() {
                    "call_expression" => todo!(),
                    "field_expression" => todo!(),
                    "identifier" => {}
                    "parenthesized_expression" => todo!(),
                    "pointer_expression" => todo!(),
                    "subscript_expression" => todo!(),
                    _ => unreachable!(),
                }

                match self.src(operator) {
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
                }

                self.expression(right, env);

                self.push('<');
                self.stack_pointer -= 1;

                let var_location = env
                    .lookup(self.src(left))
                    .expect("variable should've been found");
                let var_offset = self.stack_pointer - var_location;

                // Clear original var memory
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
                fields!(node: left, operator, right);

                let push_left = |s: &mut Codegen<'src>| match left.kind() {
                    kind if is_expression(kind) => s.expression(left, env),
                    "preproc_defined" => todo!(),
                    _ => unreachable!(),
                };

                let push_right = |s: &mut Codegen<'src>| match right.kind() {
                    kind if is_expression(kind) => s.expression(right, env),
                    "preproc_defined" => todo!(),
                    _ => unreachable!(),
                };

                match self.src(operator) {
                    "+" => {
                        push_left(self);
                        push_right(self);
                        self.push_str("<[<+>-]");

                        self.stack_pointer -= 1;
                    }
                    "-" => {
                        push_left(self);
                        push_right(self);
                        self.push_str("<[<->-]");

                        self.stack_pointer -= 1;
                    }
                    "==" => {
                        self.push_str("+>");
                        self.stack_pointer += 1;

                        push_left(self);
                        push_right(self);

                        // Subtract a - b
                        {
                            self.push_str("<[<->-]");

                            self.stack_pointer -= 1;
                        }

                        self.push('<');

                        bf_loop!(self, {
                            self.push_str("[-]");
                            self.push_str("<->");
                        });

                        self.stack_pointer -= 1;
                    }
                    "!=" => {
                        self.push_str(">");
                        self.stack_pointer += 1;

                        push_left(self);
                        push_right(self);

                        // Subtract a - b
                        {
                            self.push_str("<[<->-]");

                            self.stack_pointer -= 1;
                        }

                        self.push('<');

                        bf_loop!(self, {
                            self.push_str("[-]");
                            self.push_str("<+>");
                        });

                        self.stack_pointer -= 1;
                    }
                    _ => todo!(),
                }
            }
            "call_expression" => {
                fields!(node: function, arguments);

                self.argument_list(arguments, env);

                // TODO: Functions can be any expression
                match self.src(function) {
                    "putchar" => self.push_str("<.[-]"),
                    _ => panic!(),
                }

                self.stack_pointer -= arguments.named_child_count();
            }
            "cast_expression" => todo!(),
            "char_literal" => {
                assert!(
                    node.named_child_count() == 1,
                    "expected one character in char literal"
                );

                let child = node.named_child(0).unwrap();

                let char = match child.kind() {
                    "character" => self.src(child).chars().next().unwrap(),
                    "escape_sequence" => match self.src(child) {
                        r"\'" => '\'',
                        r#"\""# => '\"',
                        r"\?" => '?',
                        r"\\" => '\\',
                        r"\a" => '\x07',
                        r"\b" => '\x08',
                        r"\f" => '\x0c',
                        r"\n" => '\n',
                        r"\r" => '\r',
                        r"\t" => '\t',
                        r"\v" => '\x0b',
                        esc if esc.starts_with(r"\x") => todo!(),
                        esc if esc.starts_with(r"\u") => todo!(),
                        esc if esc.starts_with(r"\U") => todo!(),
                        esc if esc.starts_with('\\') => todo!(),
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
            "parenthesized_expression" => self.parenthesized_expression(node, env),
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
            "update_expression" => {
                fields!(node: argument, operator);

                debug_assert_eq!(argument.kind(), "identifier", "update expression lvalue must be an identifier");

                let var_location = env
                    .lookup(self.src(argument))
                    .expect("Variable not found");
                let var_offset = self.stack_pointer - var_location;

                self.push_n(var_offset, '<');

                self.push(match self.src(operator) {
                    "++" => '+',
                    "--" => '-',
                    _ => unreachable!(),
                });

                self.push_n(var_offset, '>');
            }
            _ => unreachable!(),
        }
    }

    /// Evaluates a parenthesized expression (most cases,
    /// this is just syntactically required or to indicate
    /// operation order in expressions) and pushes its
    /// value onto stack.
    fn parenthesized_expression(&mut self, node: Node, env: &Environment) {
        let child = node.named_child(0).unwrap();

        match child.kind() {
            "comma_expression" => todo!(),
            "compound_statement" => todo!(),
            kind if is_expression(kind) => self.expression(child, env),
            "preproc_defined" => todo!(),
            _ => unreachable!(),
        }
    }

    /// Pushes the value of each of the passed arguments
    /// onto stack sequentially.
    fn argument_list(&mut self, node: Node, env: &Environment) {
        for argument in node.named_children(&mut node.walk()) {
            match argument.kind() {
                "compound_statement" => todo!(),
                kind if is_expression(kind) => self.expression(argument, env),
                "preproc_defined" => todo!(),
                _ => unreachable!(),
            }
        }
    }
}
