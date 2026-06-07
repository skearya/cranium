//! Code generation logic for cranium.

use std::collections::HashMap;

use crate::treesitter_wrapper::*;

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
    /// Absolute location of the beginning of
    /// the local varaibles for the current scope.
    stack_base: usize,
    /// Maps variable name to absolute location and type.
    variables: HashMap<String, (usize, ValueType)>,
    /// Maps `typedef`-created type name to the `ValueType`.
    types: HashMap<String, ValueType>,
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
    fn from_type_specifier(spec: &TypeSpecifier, env: &Environment) -> Self {
        match *spec {
            TypeSpecifier::PrimitiveType(ref prim) => match prim.src.as_str() {
                "char" => Self::Char,
                "bool" => Self::Bool,
                "void" => Self::Void,
                _ => panic!("Unknown primitive type specifier encountered: {}", prim.src),
            },
            TypeSpecifier::TypeIdentifier(ref id) => {
                env.lookup_type(&id.src).expect("Type should be defined")
            }
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
            Expression::ParenthesizedExpression(ref paren_expr) => {
                Self::from_expression(&paren_expr.child, env)
            }
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
                assert!(
                    matches!(left_type, Self::Char),
                    "non-integer type used for addition or subtraction"
                );

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
    fn interpret_declarator(declarator: &Declarator, prior_type: ValueType) -> (String, ValueType) {
        match *declarator {
            Declarator::Identifier(ref id) => (id.src.clone(), prior_type),
            Declarator::InitDeclarator(ref init) => {
                interpret_declarator(&init.declarator, prior_type)
            }
            Declarator::FunctionDeclarator(_) => panic!("Unexpected function declarator"),
        }
    }

    interpret_declarator(
        &decl.declarator,
        ValueType::from_type_specifier(&decl.r#type, env),
    )
}

/// Returns the associated name and type with a `typedef` statement.
fn interpret_type_definition(typedef: &TypeDefinition, env: &Environment) -> (String, ValueType) {
    fn interpret_type_declarator(
        declarator: &TypeDeclarator,
        prior_type: ValueType,
    ) -> (String, ValueType) {
        match *declarator {
            TypeDeclarator::TypeIdentifier(ref id) => (id.src.clone(), prior_type),
            // TODO: more declarator variants (prolly just array...)
        }
    }

    interpret_type_declarator(
        &typedef.declarator,
        ValueType::from_type_specifier(&typedef.r#type, env),
    )
}

impl<'a> Environment<'a> {
    /// Creates a new environment with an optional parent.
    fn new(parent: Option<&'a Environment>) -> Self {
        Self {
            parent,
            // horrendous (-ly beautiful?) one liner
            stack_base: parent.map_or(0, |parent| {
                parent.stack_base
                    + parent
                        .variables
                        .values()
                        .fold(0, |acc, &(_loc, r#type)| acc + r#type.size())
            }),
            variables: HashMap::new(),
            types: HashMap::new(),
        }
    }

    /// Returns absolute location and type of a variable.
    fn lookup_variable(&self, name: &str) -> Option<(usize, ValueType)> {
        self.variables
            .get(name)
            .copied()
            .or(self.parent.and_then(|parent| parent.lookup_variable(name)))
    }

    /// Returns type associated with a name.
    fn lookup_type(&self, name: &str) -> Option<ValueType> {
        self.types
            .get(name)
            .copied()
            .or(self.parent.and_then(|parent| parent.lookup_type(name)))
    }

    /// Adds a `typedef` type to the environment, given its name and type panicking if it redefines a type that already exists in the current scope.
    fn add_type(&mut self, name: String, r#type: ValueType) {
        if let Some(previous_type) = self.types.insert(name.clone(), r#type)
            && previous_type != r#type
        {
            panic!("redefined {} type", &name);
        }
    }

    /// Adds a `typedef` type to the environment given its definition node.
    fn add_type_from_node(&mut self, typedef: &TypeDefinition) {
        let (name, r#type) = interpret_type_definition(typedef, self);

        self.add_type(name, r#type);
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

    /// Generates a BF loop where the code in `blk` is executed in-between pushing the loop's delimiting `[` and `]`. The closure `blk` must accept a mutable reference to the `Codegen` object which it then uses to invokes any code generation.
    fn bf_loop<F: FnOnce(&mut Self)>(&mut self, blk: F) {
        self.push('[');
        blk(self);
        self.push(']');
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

        if env.variables.contains_key(&name) {
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
        let mut top_level_env = Environment::new(None);

        for child in &root.children {
            match *child {
                TUChildren::FunctionDefinition(ref funcdef) => match *funcdef.declarator {
                    Declarator::FunctionDeclarator(ref fd) => {
                        if let Declarator::Identifier(ref func_name) = *fd.declarator
                            && func_name.src == "main"
                        {
                            match *funcdef.r#type {
                                TypeSpecifier::PrimitiveType(ref t) if t.src.as_str() == "int" => {}
                                _ => panic!("main function does not have `int` return type"),
                            }

                            // sorry it's so unprofessional i just wanted the compiler to shut up
                            fd.parameters.children.iter().for_each(|param_decl| {
                                match *param_decl.declarator {
                                    Declarator::Identifier(ref id) => println!("{} dies", &id.src),
                                    Declarator::FunctionDeclarator(_) => println!("do NOT even bother passing a function declarator as a function parameter"),
                                    Declarator::InitDeclarator(_) => println!("initialization??? in THIS signature??"),
                                }
                                match *param_decl.r#type {
                                    TypeSpecifier::PrimitiveType(ref t) => println!("ESPECIALLY if it's of type {}", &t.src),
                                    TypeSpecifier::TypeIdentifier(ref id) => println!("even with a type of {}", &id.src),
                                }
                                panic!("no chance main has any parameters");
                            });

                            self.main(funcdef, &top_level_env)
                        } else {
                            unimplemented!("non-main functions");
                        }
                    }
                    Declarator::Identifier(_) => unimplemented!(),
                    Declarator::InitDeclarator(_) => unimplemented!(),
                },
                TUChildren::TypeDefinition(ref typedef) => {
                    top_level_env.add_type_from_node(typedef)
                }
            }
        }
    }

    /// Generate code for the `main` function, which is
    /// where program execution begins.
    fn main(&mut self, function: &FunctionDefinition, env: &Environment) {
        self.compound_statement(function.body.as_ref(), env);
    }

    /// This generates code for a scoping block (known internally
    /// as a compound statement). Creates a new environment for
    /// the local variables and types declared here.
    fn compound_statement(&mut self, node: &CompoundStatement, parent_env: &Environment) {
        let mut env = Environment::new(Some(parent_env));

        for child in &node.children {
            match *child {
                BlockChild::Declaration(ref decl) => {
                    // like why both...
                    self.add_variable(&mut env, decl);
                    self.declaration(decl, &env);
                }
                BlockChild::Statement(ref stmt) => self.statement(stmt, &env),
                BlockChild::TypeDefinition(ref typedef) => env.add_type_from_node(typedef),
            }
        }

        // This stupid thing ensures that the stack is empty
        // and that all that's left are the locals.
        // Pretty sure the logic is right but you never know.
        debug_assert_eq!(
            self.stack_pointer,
            env.variables
                .values()
                .max_by(|(loc1, _type1), (loc2, _type2)| loc1.cmp(loc2))
                .map(|(loc, r#type)| *loc + r#type.size())
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
                let var_size = r#type.size();

                // push and do NOT inspect
                self.expression(&init.value, env);

                // now pointer is directly after data, e.g.:
                // xxx...yyy
                //          ^
                // size = 3
                // var_offset = 6

                // discarding type because we already established it from `interpret_declaration`.
                // i really should merge these functions but wtv
                let (var_location, _) = env.variables[&name];
                let var_offset = self.stack_pointer - var_size - var_location;

                // for each cell...
                for _ in 0..var_size {
                    // move into the cell
                    self.move_head(-1);
                    // while cell isn't empty...
                    self.bf_loop(|cg| {
                        // decrement from stack cell
                        cg.push('-');
                        // move to corresponding variable cell
                        cg.push_n(var_offset, '<');
                        // increment variable cell
                        cg.push('+');
                        // move back to stack cell
                        cg.push_n(var_offset, '>');
                    });
                }
                // now we're AT stack empty so we're chill.
            }
            Declarator::FunctionDeclarator(_) => unimplemented!(),
        }
    }

    /// Generates code for any statement.
    fn statement(&mut self, stmt: &Statement, env: &Environment<'_>) {
        match *stmt {
            Statement::CompoundStatement(ref cs) => self.compound_statement(cs, env),
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
        let mut outer_env = Environment::new(Some(env));

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

        self.bf_loop(|cg| {
            // clear cond if true
            cg.push_str("[-]");

            // common case is compound_statement;
            // in which case, new environment created,
            // which is correct behavior.
            cg.statement(&node.body, &outer_env);

            if let Some(update) = &node.update {
                let old_sp = cg.stack_pointer;

                cg.expression(update, &outer_env);

                let dist = cg.stack_pointer - old_sp;

                // maybe new move_and_clear function? or would
                // that tread too far into premature abstraction?
                cg.push_n_str(dist, "<[-]");
                cg.stack_pointer -= dist;
            }

            examine_condition(cg);
        });

        self.clear_environment(outer_env);
    }

    /// Generates code for an `if` statement.
    fn if_statement(&mut self, node: &IfStatement, env: &Environment<'_>) {
        if !matches!(
            ValueType::from_expression(&node.condition.child, env),
            ValueType::Char | ValueType::Bool
        ) {
            unimplemented!("Condition of type other than bool or char");
        }

        if let Some(alternative) = &node.alternative {
            // Init flag to 1
            self.push('+');
            self.move_head(1);

            // Examine condition
            self.parenthesized_expression(&node.condition, env);
            self.move_head(-1);

            // If cond != 0 (true), set flag = 0, eval consequence
            self.bf_loop(|cg| {
                cg.push_str("<->");
                cg.push_str("[-]");

                cg.statement(&node.consequence, env);
            });

            // Cond space guaranteed to be zero, moving to examine flag
            self.move_head(-1);

            // If flag != 0 (i.e., cond false), eval alternative
            self.bf_loop(|cg| {
                cg.push('-');

                cg.statement(&alternative.child, env);
            });
        } else {
            // Examine condition
            self.parenthesized_expression(&node.condition, env);
            self.move_head(-1);

            // If cond != 0 (true), set it to zero and eval consequence
            self.bf_loop(|cg| {
                cg.push_str("[-]");

                cg.statement(&node.consequence, env);
            });
        }
    }

    /// Generates code for a `while` statement.
    fn while_statement(&mut self, node: &WhileStatement, env: &Environment<'_>) {
        if !matches!(
            ValueType::from_expression(&node.condition.child, env),
            ValueType::Char | ValueType::Bool
        ) {
            unimplemented!("Condition of type other than bool or char");
        }

        // Examine condition
        self.parenthesized_expression(&node.condition, env);
        self.push('<');
        self.stack_pointer -= 1;

        // If cond != 0, clear and evaluate body
        self.bf_loop(|cg| {
            cg.push_str("[-]");

            cg.statement(&node.body, env);

            // Examine condition again so we can run it back
            cg.parenthesized_expression(&node.condition, env);
            cg.push('<');
            cg.stack_pointer -= 1;
        });
    }

    /// Evaluates any expression and pushes its value onto stack.
    fn expression(&mut self, expr: &Expression, env: &Environment<'_>) {
        match *expr {
            Expression::AssignmentExpression(ref ae) => self.assignment_expression(ae, env),
            Expression::BinaryExpression(ref be) => self.binary_expression(&be, env),
            Expression::CallExpression(ref ce) => {
                self.argument_list(&ce.arguments, env);

                match ce.function.src.as_str() {
                    "putchar" => {
                        self.move_head(-1);
                        self.push_str(".[-]");
                    }
                    _ => unimplemented!("Non-putchar function invocation"),
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
            Expression::UpdateExpression(ref update_expr) => {
                self.update_expression(update_expr, env)
            }
            Expression::ParenthesizedExpression(ref paren_expr) => {
                self.parenthesized_expression(paren_expr, env)
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

        // for every cell in the temp value...
        for _ in 0..var_size {
            // while the temp cell is nonzero...
            self.bf_loop(|cg| {
                // subtract from temp, add to stack destination
                cg.push_str("-<+");
                // move to local variable cell
                cg.push_n(var_dist - 1, '<');
                // add to local
                cg.push('+');
                // move back to temp value's cell and repeat
                cg.push_n(var_dist, '>');
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
                self.bf_loop(|cg| {
                    cg.push_str("[-]");
                    cg.push_str("<->");
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
                self.bf_loop(|cg| {
                    cg.push_str("[-]");
                    cg.push_str("<+>");
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
            self.bf_loop(|cg| {
                // subtract from local cell
                cg.push('-');
                // move back to stack
                cg.push_n(var_distance, '>');
                // incremement stack and temp cell
                cg.push_str("+>+");
                // move back to local
                cg.push_n(var_distance + 1, '<');
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
            self.bf_loop(|cg| {
                // subtract from temp cell
                cg.push('-');
                // move to variable
                cg.push_n(var_distance + 1, '<');
                // increment variable cell
                cg.push('+');
                // move back to temp
                cg.push_n(var_distance + 1, '>');
            });
            // advance to next cell
            self.push('>');
        }
        // move back to top of stack
        self.push_n(var_size, '<');

        // now we've moved by one var_size
        self.stack_pointer += var_size;
    }

    /// Generates code for an update expression
    ///
    /// For technical reasons, this function cannot easily ascertain whether the update operator was prefixed or postfixed so it currently assumes it's postfixed.
    fn update_expression(&mut self, update_expr: &UpdateExpression, env: &Environment) {
        // TODO: this thing currently just assumes the update was postfixed.
        // there's no way to structurally check in the AST whether
        // it was prefixed or postfixed, which SUCKS so i'd have
        // to probably check sourcecode

        let dist = match *update_expr.argument {
            Expression::Identifier(ref id) => {
                let (var_location, r#type) = env
                    .lookup_variable(&id.src)
                    .expect("Variable should have been defined");

                // this function is majorly uninvolved from the type system, sadly
                if r#type != ValueType::Char {
                    unimplemented!("Non-integer types not supported for update expressions");
                }

                self.stack_pointer - var_location
            }
            _ => unimplemented!("Non-identifiers not implemented for update expressions"),
        };

        // make space for final stack value
        self.move_head(1);

        // move value from variable to temp and inspect
        self.push_n(dist + 1, '<');
        self.bf_loop(|cg| {
            cg.push('-');
            cg.push_n(dist + 1, '>');
            cg.push('+');
            cg.push_n(dist + 1, '<');
        });
        self.push_n(dist + 1, '>');

        // update temp according to operator
        self.push(match *update_expr.operator {
            UpdateOperator::PlusPlus => '+',
            UpdateOperator::MinusMinus => '-',
        });

        // copy into variable and to stack
        self.bf_loop(|cg| {
            cg.push_str("-<+");
            cg.push_n(dist, '<');
            cg.push('+');
            cg.push_n(dist + 1, '>');
        });

        // we are now after the stack value, so we're done!
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
