mod codegen;
mod interpreter;
mod macros;

use tree_sitter::Parser;

use crate::codegen::Codegen;

fn main() {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .expect("Error loading C parser");

    let source = include_str!("../input.c");

    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    let output = Codegen::new(source).generate(&root);

    println!("codegen = \"{output}\"");

    interpreter::run(&output);
}
