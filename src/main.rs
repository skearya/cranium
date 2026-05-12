mod codegen;
mod interpreter;
mod macros;
mod treesitter_wrapper;

use crate::codegen::Codegen;

fn main() {
    let source = include_str!("../input.c");

    let output = Codegen::new(source).generate();

    println!("codegen = \"{output}\"");

    interpreter::run(&output);
}
