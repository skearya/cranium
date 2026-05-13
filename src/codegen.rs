//! Code generation logic for cranium.

use std::collections::HashMap;

use crate::treesitter_wrapper as ts;
use ts::*;

use crate::macros::bf_loop;

/// Stateful type keeping track of the C to BF code
/// generation.
pub struct Codegen {
    /// Source C code.
    src: String,
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

impl<'src> Codegen {
    /// Initializes a `Codegen` object given C source code.
    pub fn new(src: &str) -> Self {
        Self {
            src: src.to_string(),
            stack_pointer: 0,
            output: String::new(),
        }
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
    fn add_variable(&mut self, env: &mut Environment<'src, '_>, declaration: &'src Declaration) {
        // Sizes over 1 coming soon...
        let size: usize = match *declaration.r#type {
            TypeSpecifier::PrimitiveType(ref pt) => match pt.src.as_str() {
                "char" | "bool" => 1,
                _ => unimplemented!(),
            },
        };

        let name = match *declaration.declarator {
            Declarator::Identifier(ref id) => id.src.as_str(),
            Declarator::InitDeclarator(ref init) => match *init.declarator {
                Declarator::Identifier(ref id) => id.src.as_str(),
                Declarator::InitDeclarator(_) => unimplemented!(),
                Declarator::FunctionDeclarator(_) => unimplemented!(),
            },
            Declarator::FunctionDeclarator(_) => unimplemented!(),
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
        let translation_unit = parse(self.src.as_str());

        self.translation_unit(&translation_unit);

        self.output
    }

    /// Generate code for a `translation_unit` node.
    ///
    /// For the purposes of this project this refers to
    /// a parsed C file.
    fn translation_unit(&mut self, root: &TranslationUnit) {
        for child in &root.children {
            match *child.declarator {
                Declarator::FunctionDeclarator(ref fd) => {
                    if let Declarator::Identifier(ref func_name) = *fd.declarator
                        && func_name.src == "main"
                    {
                        match *child.r#type {
                            TypeSpecifier::PrimitiveType(ref t) if t.src.as_str() == "int" => {},
                            _ => panic!("main function does not have `int` return type"),
                        }

                        // sorry it's so unprofessional i just wanted the compiler to shut up
                        fd.parameters.children.iter().for_each(|x| {
                            match *x.declarator {
                                Declarator::Identifier(ref id) => println!("{} dies", id.src),
                                Declarator::FunctionDeclarator(_) => println!("do NOT even bother passing a function declarator as a function parameter"),
                                Declarator::InitDeclarator(_) => println!("initialization??? in THIS signature??"),
                            }
                            match *x.r#type {
                                TypeSpecifier::PrimitiveType(ref t) => println!("ESPECIALLY if it's of type {}", t.src),
                            }
                            panic!("no chance main has any parameters");
                        });

                        self.main(child)
                    } else {
                        unimplemented!()
                    }
                }
                Declarator::Identifier(_) => unimplemented!(),
                Declarator::InitDeclarator(_) => unimplemented!(),
            }
        }
    }

    /// Generate code for the `main` function, which is
    /// where program execution begins.
    fn main(&mut self, function: &FunctionDefinition) {
        self.compound_statement(function.body.as_ref(), None);
    }

    /// This generates code for a scoping block (known internally
    /// as a compound statement). Creates a new environment for
    /// the local variables declared here.
    fn compound_statement(
        &mut self,
        node: &CompoundStatement,
        parent: Option<&Environment<'src, '_>>,
    ) {
        let mut env = Environment {
            parent,
            variables: HashMap::new(),
            stack_base: self.stack_pointer,
        };

        for declaration in node.children.iter().filter_map(|x| match x {
            BlockChildren::Declaration(d) => Some(d),
            _ => None,
        }) {
            self.add_variable(&mut env, declaration);
        }

        for child in &node.children {
            match child {
                BlockChildren::Declaration(d) => self.declaration(d, &env),
                BlockChildren::Statement(s) => self.statement(s, &env),
            }
        }

        // This stupid thing ensures that the stack is empty
        // and that all that's left are the locals.
        // Pretty sure the logic is right but you never know.
        debug_assert_eq!(
            self.stack_pointer,
            env.variables
                .values()
                .max()
                .map(|&x| x + 1)
                .unwrap_or(env.stack_base),
            "Stack not empty on scope exit"
        );

        self.clear_environment(env);
    }

    /// Generates code for a variable declaration, assuming
    /// the environment already has an assigned location for it.
    fn declaration(&mut self, node: &Declaration, env: &Environment<'src, '_>) {
        // type system here we come
        match *node.r#type {
            TypeSpecifier::PrimitiveType(ref pt) => match pt.src.as_str() {
                "bool" | "char" => {}
                _ => todo!(),
            },
        }

        match *node.declarator {
            Declarator::Identifier(_) => {}
            Declarator::InitDeclarator(ref init) => {
                self.expression(&init.value, env);

                // size-dependent
                self.push('<');
                self.stack_pointer -= 1;

                let var_location = env.variables[match *init.declarator {
                    Declarator::Identifier(ref id) => id.src.as_str(),
                    Declarator::InitDeclarator(_) => unimplemented!(),
                    Declarator::FunctionDeclarator(_) => unimplemented!(),
                }];
                let var_offset = self.stack_pointer - var_location;

                bf_loop!(self, {
                    self.push_n(var_offset, '<');
                    self.push('+');
                    self.push_n(var_offset, '>');
                    self.push('-');
                });
            }
            Declarator::FunctionDeclarator(_) => unimplemented!(),
        }
    }

    /// Generates code for any statement.
    fn statement(&mut self, node: &Statement, env: &Environment<'src, '_>) {
        match *node {
            Statement::CompoundStatement(ref cs) => self.compound_statement(cs, Some(env)),
            Statement::ExpressionStatement(ref es) => {
                let child = &es.child;

                let old_stack_top = self.stack_pointer;

                self.expression(child, env);

                let clear_zone_size = self.stack_pointer - old_stack_top;
                for _ in 0..clear_zone_size {
                    self.push_str("<[-]");
                    self.stack_pointer -= 1;
                }
            }
            Statement::ForStatement(ref fs) => self.for_statement(fs, env),
            Statement::IfStatement(ref is) => self.if_statement(is, env),
            Statement::WhileStatement(ref ws) => self.while_statement(ws, env),
        }
    }

    /// Generates code for a `for` statement.
    fn for_statement(&mut self, node: &ForStatement, env: &Environment<'src, '_>) {
        // The environment wherein the for loop expressions/statements exist
        let mut outer_env = Environment {
            parent: Some(env),
            variables: HashMap::new(),
            stack_base: self.stack_pointer,
        };

        if let Some(initializer) = &node.initializer {
            match **initializer {
                ForLoopInitializer::Declaration(ref d) => {
                    self.add_variable(&mut outer_env, d);

                    self.declaration(d, &outer_env);
                }
                ForLoopInitializer::Expression(ref e) => {
                    let old_sp = self.stack_pointer;

                    self.expression(e, &outer_env);

                    let dist = self.stack_pointer - old_sp;
                    self.push_n_str(dist, "<[-]");
                    self.stack_pointer -= dist;
                }
            }
        }

        // pushes condition then moves head back so it's examining it
        let examine_condition = |cg: &mut Self| {
            match node.condition {
                Some(ref cond) => {
                    cg.expression(cond, &outer_env);

                    cg.push('<');
                    cg.stack_pointer -= 1;
                }
                // always true
                None => cg.push('+'),
            }
        };

        examine_condition(self);

        bf_loop!(self, {
            // clear cond if true
            self.push_str("[-]");

            // common case is compound_statement;
            // in which case, new environment created,
            // which is correct behavior.
            self.statement(&node.body, &outer_env);

            if let Some(update) = &node.update {
                let old_sp = self.stack_pointer;

                self.expression(update, &outer_env);

                let dist = self.stack_pointer - old_sp;
                self.push_n_str(dist, "<[-]");
                self.stack_pointer -= dist;
            }

            examine_condition(self);
        });

        self.clear_environment(outer_env);
    }

    /// Generates code for an `if` statement.
    fn if_statement(&mut self, node: &IfStatement, env: &Environment<'src, '_>) {
        if let Some(alternative) = &node.alternative {
            // Init flag to 1
            self.push('+');
            self.push('>');
            self.stack_pointer += 1;

            // Examine condition
            self.parenthesized_expression(&node.condition, env);
            self.push('<');
            self.stack_pointer -= 1;

            // If cond != 0, set flag = 0, eval consequence
            bf_loop!(self, {
                self.push_str("<->");
                self.push_str("[-]");

                self.statement(&node.consequence, env);
            });

            // Cond space guaranteed to be zero, moving to examine flag
            self.push('<');
            self.stack_pointer -= 1;

            // If flag != 0 (i.e., cond not satisfied), eval alternative
            bf_loop!(self, {
                self.push('-');

                self.statement(&alternative.child, env);
            });
        } else {
            // Examine condition
            self.parenthesized_expression(&node.condition, env);
            self.push('<');
            self.stack_pointer -= 1;

            // If cond != 0, set zero and eval consequence
            bf_loop!(self, {
                self.push_str("[-]");

                self.statement(&node.consequence, env);
            });
        }
    }

    /// Generates code for a `while` statement.
    fn while_statement(&mut self, node: &WhileStatement, env: &Environment<'src, '_>) {
        // Examine condition
        self.parenthesized_expression(&node.condition, env);
        self.push('<');
        self.stack_pointer -= 1;

        // If cond != 0, clear and evaluate body
        bf_loop!(self, {
            self.push_str("[-]");

            self.statement(&node.body, env);

            // Examine condition again so we can run it back
            self.parenthesized_expression(&node.condition, env);
            self.push('<');
            self.stack_pointer -= 1;
        });
    }

    /// Evaluates any expression and pushes its value onto stack.
    fn expression(&mut self, node: &Expression, env: &Environment<'src, '_>) {
        match *node {
            Expression::AssignmentExpression(ref ae) => self.assignment_expression(ae, env),
            Expression::BinaryExpression(ref be) => self.binary_expression(&be, env),
            Expression::CallExpression(ref ce) => {
                self.argument_list(&ce.arguments, env);

                match *ce.function {
                    Expression::Identifier(ref id) => match id.src.as_str() {
                        "putchar" => {
                            self.push_str("<.[-]");
                            self.stack_pointer -= 1;
                        }
                        _ => unimplemented!(),
                    },
                    Expression::AssignmentExpression(_)
                    | Expression::BinaryExpression(_)
                    | Expression::CallExpression(_)
                    | Expression::CharLiteral(_)
                    | Expression::False
                    | Expression::NumberLiteral(_)
                    | Expression::UpdateExpression(_) 
                    | Expression::True => unimplemented!(),
                }
            }
            Expression::CharLiteral(ref cl) => self.char_literal_expression(cl),
            Expression::False => {
                self.push('>');
                self.stack_pointer += 1;
            }
            Expression::Identifier(ref id) => self.identifier(id, env),
            Expression::NumberLiteral(ref nl) => {
                let num = nl.src.parse::<usize>().unwrap();

                self.push_n(num, '+');
                self.push('>');
                self.stack_pointer += 1;
            }
            Expression::True => {
                self.push_str("+>");
                self.stack_pointer += 1;
            }
            Expression::UpdateExpression(ref ue) => {
                let dist = match *ue.argument {
                    Expression::Identifier(ref id) => {
                        let var_location = env.lookup(&id.src).expect("Variable should have been defined");

                        self.stack_pointer - var_location
                    }
                    _ => unimplemented!(),
                };

                self.push_n(dist, '<');
                self.push(match *ue.operator {
                    UpdateOperator::PlusPlus => '+',
                    UpdateOperator::MinusMinus => '-',
                });
                self.push_n(dist, '>');

            }
        }
    }

    /// Evaluates an assignment expression, modifying lvalue
    /// and pushing rvalue onto stack.
    fn assignment_expression(
        &mut self,
        node: &AssignmentExpression,
        env: &Environment<'src, '_>,
    ) {
        // currently only supporting `id = expr` (no subscript etc)

        // space for stack value
        self.push('>');
        self.stack_pointer += 1;

        // evaluate and examine
        match *node.operator {
            AssignmentOperator::AssignEquals => {
                self.expression(&node.right, env);
                self.push('<');
                self.stack_pointer -= 1;
            }
            AssignmentOperator::PlusEquals => {
                // push onto stack
                self.identifier(&node.left, env);
                self.expression(&node.right, env);

                // add & examine
                self.push_str("<[-<+>]<");
                self.stack_pointer -= 2;
            }
            AssignmentOperator::MinusEquals => {
                // push onto stack
                self.identifier(&node.left, env);
                self.expression(&node.right, env);

                // sub & examine
                self.push_str("<[-<->]<");
                self.stack_pointer -= 2;
            }
        }

        let var_location = env
            .lookup(node.left.src.as_str())
            .expect("variable should have been declared prior to assignment");
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
        
        // Now stack pointer is after first `right`, where it should be!
    }

    /// Evaluates and pushes onto stack a binary expression's
    /// value.
    fn binary_expression(&mut self, node: &BinaryExpression, env: &Environment<'src, '_>) {
        // this is a pretty big function, not sure how to shrink it
        let push_left = |cg: &mut Self| cg.expression(&node.left, env);

        let push_right = |cg: &mut Self| cg.expression(&node.right, env);

        match *node.operator {
            BinaryOperator::EqualsCheck => {
                // set flag to 1
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

                // if difference != 0 (they are NOT equal),
                // clear diff and set flag to 0
                bf_loop!(self, {
                    self.push_str("[-]");
                    self.push_str("<->");
                });

                self.stack_pointer -= 1;
            }
            BinaryOperator::Minus => {
                push_left(self);
                push_right(self);
                self.push_str("<[<->-]");

                self.stack_pointer -= 1;
            }
            BinaryOperator::NotEqualsCheck => {
                // set flag = 0
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

                // if difference != 0 (they ARE unequal)
                // clear difference and set flag to one
                bf_loop!(self, {
                    self.push_str("[-]");
                    self.push_str("<+>");
                });

                self.stack_pointer -= 1;
            }
            BinaryOperator::Plus => {
                push_left(self);
                push_right(self);
                self.push_str("<[<+>-]");

                self.stack_pointer -= 1;
            }
        }
    }

    /// Evaluates and pushes onto stack a character's
    /// corresponding value.
    fn char_literal_expression(&mut self, node: &CharLiteral) {
        assert!(
            node.children.len() == 1,
            "expected one character in char literal"
        );

        let child = node.children.first().unwrap();

        let c = match *child {
            CharLiteralChildren::Character(ref c) => c.src.chars().next().unwrap(),
            CharLiteralChildren::EscapeSequence(ref es) => match es.src.as_str() {
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
                esc if esc.starts_with(r"\x") => unimplemented!(),
                esc if esc.starts_with(r"\u") => unimplemented!(),
                esc if esc.starts_with(r"\U") => unimplemented!(),
                esc if esc.starts_with('\\') => unimplemented!(),
                _ => unreachable!(),
            },
        };

        self.push_n(c as usize, '+');
        self.push('>');
        self.stack_pointer += 1;
    }

    /// Looks up variable in `env` and pushes its value to stack.
    fn identifier(&mut self, node: &Identifier, env: &Environment<'src, '_>) {
        let var_location = env.lookup(node.src.as_str()).expect("variable should've been found");
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

    /// Evaluates a parenthesized expression (most cases,
    /// this is just syntactically required or to indicate
    /// operation order in expressions) and pushes its
    /// value onto stack.
    fn parenthesized_expression(
        &mut self,
        node: &ParenthesizedExpression,
        env: &Environment<'src, '_>,
    ) {
        self.expression(&node.child, env);
    }

    /// Pushes the value of each of the passed arguments
    /// onto stack sequentially.
    fn argument_list(&mut self, node: &ArgumentList, env: &Environment<'src, '_>) {
        for argument in &node.children {
            self.expression(argument, env);
        }
    }
}
