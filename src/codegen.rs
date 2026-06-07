//! Code generation logic for cranium.

use std::collections::HashMap;

use crate::treesitter_wrapper::*;

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
struct Environment<'a> {
    /// The parent environment.
    parent: Option<&'a Environment<'a>>,
    /// Maps variable name to absolute location and type.
    variables: HashMap<String, (usize, ValueType)>,
    /// Absolute location of the beginning of
    /// the local varaibles for the current scope.
    stack_base: usize,
}

#[derive(Clone, Copy, PartialEq)]
/// The type associated with a value.
enum ValueType {
    /// Void type (no return). Size 0.
    Void,
    /// Boolean type (true or false). Size 1.
    Bool,
    /// The 8-bit integer type. Unsigned, but that's contentious.
    // TODO: real C chars are signed by default
    Char,
    // TODO: structs, typedefs, etc.
}

impl ValueType {
    /// Returns the `ValueType` associated with a type specifier node and the environment it occured within.
    fn from_type_specifier(spec: &TypeSpecifier, _env: &Environment) -> Self {
        match *spec {
            TypeSpecifier::PrimitiveType(ref prim) => match prim.src.as_str() {
                "char" => Self::Char,
                "bool" => Self::Bool,
                "void" => Self::Void,
                _ => panic!("Unknown primitive type specifier encountered: {}", prim.src),
            },
        }
    }

    /// Gets the type associated with an expression given the environment it occurred within.
    fn from_expression(expr: &Expression, env: &Environment) -> Self {
        match *expr {
            Expression::Identifier(ref id) => {
                env.lookup_variable(&id.src)
                    .expect("Variable should be defined")
                    .1
            }
            Expression::AssignmentExpression(ref it) => {
                env.lookup_variable(&it.left.src)
                    .expect("Variable should be defined")
                    .1
            }
            Expression::BinaryExpression(ref binexpr) => Self::from_binary_expression(binexpr, env),
            // if i ever do functions this will be a bit more involved
            Expression::CallExpression(ref call) => match call.function.src.as_str() {
                "putchar" => Self::Void,
                _ => unimplemented!("Only supporting putchar function"),
            },
            Expression::CharLiteral(_) | Expression::NumberLiteral(_) => Self::Char,
            Expression::True | Expression::False => Self::Bool,
            // these guys still disgust me
            Expression::UpdateExpression(_) => Self::Char,
        }
    }

    /// Returns the result type of a binary expression occuring within `env`.
    fn from_binary_expression(binary_expr: &BinaryExpression, env: &Environment) -> Self {
        let left_type = Self::from_expression(&binary_expr.left, env);
        let right_type = Self::from_expression(&binary_expr.right, env);

        if left_type != right_type {
            unimplemented!("not doing cross-type operators");
        }

        match *binary_expr.operator {
            BinaryOperator::EqualsCheck | BinaryOperator::NotEqualsCheck => Self::Bool,
            BinaryOperator::Plus | BinaryOperator::Minus => {
                // integer types only.
                // in C, bools can also do this
                // because they dont exist and are
                // really just 1-byte integers
                // which is silly but idc and im not gonna care
                assert!(matches!(left_type, Self::Char), "non-integer type used for addition or subtraction");

                Self::Char
            }
        }
    }

    /// Returns the size (in bytes) of the `ValueType`.
    fn size(&self) -> usize {
        match *self {
            Self::Void => 0,
            Self::Bool => 1,
            Self::Char => 1,
        }
    }
}

/// Takes a declaration node and the environment it encounters in and returns the name and type it's associated with.
fn interpret_declaration(decl: &Declaration, env: &Environment) -> (String, ValueType) {
    /// Takes a declarator node, the type it was associated with, and the environment it occurred within and returns the associated name and type for the declarator.
    fn interpret_declarator(
        declarator: &Declarator,
        prior_type: ValueType,
        env: &Environment,
    ) -> (String, ValueType) {
        match *declarator {
            Declarator::Identifier(ref id) => (id.src.clone(), prior_type),
            Declarator::InitDeclarator(ref init) => {
                interpret_declarator(&init.declarator, prior_type, env)
            }
            Declarator::FunctionDeclarator(_) => panic!("Unexpected function declarator"),
        }
    }

    interpret_declarator(
        &decl.declarator,
        ValueType::from_type_specifier(&decl.r#type, env),
        env,
    )
}

impl Environment<'_> {
    /// Returns absolute location and type of a variable.
    fn lookup_variable(&self, name: &str) -> Option<(usize, ValueType)> {
        self.variables
            .get(name)
            .copied()
            .or(self.parent.and_then(|parent| parent.lookup_variable(name)))
    }
}

impl Codegen {
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

    /// Moves the memory head by a distance either to the left
    /// (`n < 0`) or to the right (`n > 0`). This generates
    /// BF code and moves the codegen's stack pointer simultaneously.
    fn move_head(&mut self, n: isize) {
        let magnitude = n.unsigned_abs();

        match n {
            ..0 => {
                self.stack_pointer -= magnitude;

                self.push_n(magnitude, '<');
            }
            0 => {} // why?
            1.. => {
                self.stack_pointer += magnitude;

                self.push_n(magnitude, '>');
            }
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
    fn add_variable(&mut self, env: &mut Environment, decl: &Declaration) {
        let (name, r#type) = interpret_declaration(decl, env);

        if env.lookup_variable(&name).is_some() {
            panic!("Colliding variable declaration");
        }

        env.variables.insert(name, (self.stack_pointer, r#type));

        self.move_head(r#type.size().cast_signed());
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
                            TypeSpecifier::PrimitiveType(ref t) if t.src.as_str() == "int" => {}
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
    fn compound_statement(&mut self, node: &CompoundStatement, parent: Option<&Environment<'_>>) {
        let mut env = Environment {
            parent,
            variables: HashMap::new(),
            stack_base: self.stack_pointer,
        };

        for declaration in node.children.iter().filter_map(|x| match x {
            BlockChildren::Declaration(d) => Some(d),
            _ => None,
        }) {
            // TODO: problematic add_variable call...
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
                .max_by(|(loc1, _), (loc2, _)| loc1.cmp(loc2))
                .map(|&(loc, ref r#type)| loc + r#type.size())
                .unwrap_or(env.stack_base),
            "Stack not empty on scope exit"
        );

        self.clear_environment(env);
    }

    /// Generates code for a variable declaration, assuming
    /// the environment already has an assigned location for it.
    // TODO: Merge this and `add_variable`, they feel like they should just be the same thing.
    fn declaration(&mut self, decl: &Declaration, env: &Environment<'_>) {
        match *decl.declarator {
            Declarator::Identifier(_) => {}
            Declarator::InitDeclarator(ref init) => {
                let (name, r#type) = interpret_declaration(decl, env);

                // TODO: type casting?
                let expr_type = ValueType::from_expression(&init.value, env);
                if r#type != expr_type {
                    unimplemented!("Assigning values of incompatible types");
                }
                self.expression(&init.value, env);

                // discarding type because we already established it from `interpret_declaration`.
                // i really should merge these functions but wtv
                let (var_location, _) = env.variables[&name];
                let var_offset = self.stack_pointer - var_location;

                for _ in 0..r#type.size() {
                    self.move_head(-1);

                    // copying values
                    bf_loop!(self, {
                        self.push_n(var_offset, '<');
                        self.push('+');
                        self.push_n(var_offset, '>');
                        self.push('-');
                    });
                }
            }
            Declarator::FunctionDeclarator(_) => unimplemented!(),
        }
    }

    /// Generates code for any statement.
    fn statement(&mut self, node: &Statement, env: &Environment<'_>) {
        match *node {
            Statement::CompoundStatement(ref cs) => self.compound_statement(cs, Some(env)),
            Statement::ExpressionStatement(ref es) => {
                let child = &es.child;

                let old_stack_top = self.stack_pointer;

                self.expression(child, env);

                let clear_zone_size = self.stack_pointer - old_stack_top;
                for _ in 0..clear_zone_size {
                    self.move_head(-1);
                    self.push_str("[-]");
                }
            }
            Statement::ForStatement(ref fs) => self.for_statement(fs, env),
            Statement::IfStatement(ref is) => self.if_statement(is, env),
            Statement::WhileStatement(ref ws) => self.while_statement(ws, env),
        }
    }

    /// Generates code for a `for` statement.
    fn for_statement(&mut self, node: &ForStatement, env: &Environment<'_>) {
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
                    cg.move_head(-1);
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

                // maybe new move_and_clear function? or would
                // that tread too far into premature abstraction?
                self.push_n_str(dist, "<[-]");
                self.stack_pointer -= dist;
            }

            examine_condition(self);
        });

        self.clear_environment(outer_env);
    }

    /// Generates code for an `if` statement.
    fn if_statement(&mut self, node: &IfStatement, env: &Environment<'_>) {
        if let Some(alternative) = &node.alternative {
            // Init flag to 1
            self.push('+');
            self.move_head(1);

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
    fn while_statement(&mut self, node: &WhileStatement, env: &Environment<'_>) {
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
    fn expression(&mut self, node: &Expression, env: &Environment<'_>) {
        match *node {
            Expression::AssignmentExpression(ref ae) => self.assignment_expression(ae, env),
            Expression::BinaryExpression(ref be) => self.binary_expression(&be, env),
            Expression::CallExpression(ref ce) => {
                self.argument_list(&ce.arguments, env);

                match ce.function.src.as_str() {
                    "putchar" => {
                        self.push_str("<.[-]");
                        self.stack_pointer -= 1;
                    }
                    _ => unimplemented!("Only supporting putchar function"),
                }
            }
            Expression::CharLiteral(ref cl) => self.char_literal_expression(cl),
            Expression::Identifier(ref id) => self.identifier(id, env),
            Expression::NumberLiteral(ref nl) => {
                let num = nl.src.parse::<usize>().unwrap();

                self.push_n(num, '+');
                self.move_head(1);
            }
            Expression::True => {
                self.push('+');
                self.move_head(1);
            }
            Expression::False => self.move_head(1),
            Expression::UpdateExpression(ref ue) => {
                // TODO: this thing just assumes the update was postfixed.
                // there's no way to structurally check in the AST whether
                // it was prefixed or postfixed, which SUCKS so i'd have
                // to probably check sourcecode

                let dist = match *ue.argument {
                    Expression::Identifier(ref id) => {
                        let (var_location, r#type) = env
                            .lookup_variable(&id.src)
                            .expect("Variable should have been defined");

                        // this function is majorly uninvolved from the type system, sadly
                        if r#type != ValueType::Char {
                            unimplemented!(
                                "Non-integer types not supported for update expressions"
                            );
                        }

                        self.stack_pointer - var_location
                    }
                    _ => unimplemented!("Non-identifiers not implemented for update expressions"),
                };

                // make space for final stack value
                self.move_head(1);

                // move and inspect
                self.push_n(dist + 1, '<');
                bf_loop!(self, {
                    self.push('-');
                    self.push_n(dist + 1, '>');
                    self.push('+');
                    self.push_n(dist + 1, '<');
                });
                self.push_n(dist + 1, '>');

                // update
                self.push(
                match *ue.operator {
                    UpdateOperator::PlusPlus => '+',
                    UpdateOperator::MinusMinus => '-',
                });

                // copy into variable and to stack
                bf_loop!(self, {
                    self.push_str("-<+");
                    self.push_n(dist, '<');
                    self.push('+');
                    self.push_n(dist + 1, '>');
                });

                // we are now after the stack value, so we're done!
            }
        }
    }

    /// Evaluates an assignment expression, modifying lvalue
    /// and pushing rvalue onto stack.
    fn assignment_expression(&mut self, node: &AssignmentExpression, env: &Environment<'_>) {
        // currently only supporting `id (=|+=|-=) expr` (no subscript etc)
        // TODO: lvalue evaluation for subscript, struct access, etc

        let (location, r#type) = env
            .lookup_variable(&node.left.src)
            .expect("Variable should be defined");

        let var_size = r#type.size();

        // space for stack value
        self.move_head(var_size.cast_signed());

        // evaluate and examine
        match *node.operator {
            AssignmentOperator::AssignEquals => {
                self.expression(&node.right, env);
                self.move_head(var_size.cast_signed() * -1);
            }

            // with both += and -= we're only doing integers (which are currently just chars)
            // so we can safely assume size = 1.
            
            AssignmentOperator::PlusEquals => {
                if r#type != ValueType::Char {
                    unimplemented!("Non-integer types not doing plus-equals assignment");
                }
                
                // push onto stack
                self.identifier(&node.left, env);
                self.expression(&node.right, env);

                // add & examine
                self.move_head(-1);
                self.push_str("[-<+>]");
                self.move_head(-1);
            }
            AssignmentOperator::MinusEquals => {
                if r#type != ValueType::Char {
                    unimplemented!("Non-integer types not doing minus-equals assignment");
                }

                // push onto stack
                self.identifier(&node.left, env);
                self.expression(&node.right, env);

                // sub & examine
                self.move_head(-1);
                self.push_str("[-<->]");
                self.move_head(-1);
            }
        }

        let var_dist = self.stack_pointer - location;

        // clear original var memory
        self.push_n(var_dist, '<');
        for _ in 0..var_size {
            self.push_str("[-]>");
        }
        self.push_n(var_dist - var_size, '>');

        // copy into stack value and local variable

        // this loop iterates over every cell in the temp value
        for _ in 0..var_size {
            bf_loop!(self, {
                // subtract from temp, add to stack destination
                self.push_str("-<+");
                // move to local variable cell
                self.push_n(var_dist - 1, '<');
                // add to local
                self.push('+');
                // move back to temp value's cell
                self.push_n(var_dist, '>');
            });
            // advance to next cell of temp
            self.push('>');
        }
        self.push('<');

        // Now stack pointer is after first `right`, where it should be!
    }

    /// Evaluates and pushes onto stack a binary expression's
    /// value.
    fn binary_expression(&mut self, node: &BinaryExpression, env: &Environment<'_>) {
        // this is a pretty big function, not sure how to shrink it

        let push_left = |cg: &mut Self| cg.expression(&node.left, env);
        let push_right = |cg: &mut Self| cg.expression(&node.right, env);

        let left_type = ValueType::from_expression(&node.left, env);
        let right_type = ValueType::from_expression(&node.right, env);

        if left_type != right_type {
            unimplemented!("Binary operators across types");
        }
        
        match *node.operator {
            // TODO: equality for all types
            BinaryOperator::EqualsCheck => {
                if !matches!(left_type, ValueType::Bool | ValueType::Char) {
                    todo!("Equality checks for types outside of char and bool");
                }
                
                // set flag to 1
                self.push('+');
                self.move_head(1);

                push_left(self);
                push_right(self);

                // Subtract a - b
                self.move_head(-1);
                self.push_str("[-<->]");
                // inspect difference (2nd value is cleared)
                self.move_head(-1);

                // if difference != 0 (they are NOT equal),
                // clear diff and set flag to 0
                bf_loop!(self, {
                    self.push_str("[-]");
                    self.push_str("<->");
                });
            }
            BinaryOperator::NotEqualsCheck => {
                // set flag = 0
                self.move_head(1);

                push_left(self);
                push_right(self);

                // Subtract a - b
                {
                    self.push_str("<[<->-]");

                    self.stack_pointer -= 1;
                }

                // inspect result
                self.move_head(-1);

                // if difference != 0 (they ARE unequal)
                // clear difference and set flag to one
                bf_loop!(self, {
                    self.push_str("[-]");
                    self.push_str("<+>");
                });
            }
            BinaryOperator::Plus => {
                push_left(self);
                push_right(self);
                self.move_head(-1);
                self.push_str("[<+>-]");
            }
            BinaryOperator::Minus => {
                push_left(self);
                push_right(self);
                self.move_head(-1);
                self.push_str("[<->-]");
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
        self.move_head(1);
    }

    /// Looks up variable in `env` and pushes its value to stack.
    fn identifier(&mut self, node: &Identifier, env: &Environment<'_>) {
        let (var_location, var_type) = env
            .lookup_variable(node.src.as_str())
            .expect("variable should've been found");
        let var_distance = self.stack_pointer - var_location;
        let var_size = var_type.size();

        // Copy to two locations: stack and temp (adjacent)

        // move to var location
        self.push_n(var_distance, '<');
        // for each of the local's cells...
        for _ in 0..var_size {
            // until cell empty...
            bf_loop!(self, {
                // subtract from local cell
                self.push('-');
                // move back to stack
                self.push_n(var_distance, '>');
                // incremement stack and temp cell
                self.push_str("+>+");
                // move back to local
                self.push_n(var_distance + 1, '<');
            });
            // now move to next cell
            self.push('>');
        }
        // move back to temp
        self.push_n(var_distance, '>');

        // Move temp back into source

        // for each temp cell...
        for _ in 0..var_size {
            // while temp cell isn't empty...
            bf_loop!(self, {
                // subtract from temp cell
                self.push('-');
                // move to variable
                self.push_n(var_distance + 1, '<');
                // increment variable cell
                self.push('+');
                // move back to temp
                self.push_n(var_distance + 1, '>');
            });
            // advance to next cell
            self.push('>');
        }
        // move back to top of stack
        self.push_n(var_size, '<');

        // now we've moved by one var_size
        self.stack_pointer += var_size;
    }

    /// Evaluates a parenthesized expression (most cases,
    /// this is just syntactically required or to indicate
    /// operation order in expressions) and pushes its
    /// value onto stack.
    fn parenthesized_expression(&mut self, node: &ParenthesizedExpression, env: &Environment<'_>) {
        self.expression(&node.child, env);
    }

    /// Pushes the value of each of the passed arguments
    /// onto stack sequentially.
    fn argument_list(&mut self, node: &ArgumentList, env: &Environment<'_>) {
        for argument in &node.children {
            self.expression(argument, env);
        }
    }
}
