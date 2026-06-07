# cranium

C compiler targeting [brainfuck](https://brainfuck.org/), an 8-instruction language faithful to the original Turing Machine.

Written in Rust, this program utilizes [tree-sitter](https://github.com/tree-sitter/tree-sitter) and [tree_sitter_c](https://docs.rs/tree-sitter-c/latest/tree_sitter_c/) for parsing.

This repository includes an in-house brainfuck interpreter with debug capabilities as well as a complete strictly typed wrapper around the untyped tree-sitter Rust API.

## Features

* `main` function definition
* `putchar` to print a character
* `char`, `bool`, `void` types
* `typedef`
* local variables
* `if` and `else` statements
* `while` statements
* `for` statements
* `+`, `-`, `++`, `--` operators
* `==` and `!=` check operators

## Example

```c
typedef bool check_t;

int main() {
  for (char i = 0; i != 3; i++) {
    check_t check = i == 2;
    if (check) {
      putchar('a');
    }

    putchar('h' + i);
  }
}
```

becomes...

```bf
>><[-<+>]><<[->>+>+<<<]>>>[-<<<+
>>>]><+++><[<->-]<[[-]<+>]<[[-]>
+><<<[->>>+>+<<<<]>>>>[-<<<<+>>>
>]><++><[-<->]<[[-]<->]<[-<+>]<[
->+>+<<]>>[-<<+>>]><<[[-]+++++++
++++++++++++++++++++++++++++++++
++++++++++++++++++++++++++++++++
++++++++++++++++++++++++++><.[-]
]+++++++++++++++++++++++++++++++
++++++++++++++++++++++++++++++++
++++++++++++++++++++++++++++++++
+++++++++><<<[->>>+>+<<<<]>>>>[-
<<<<+>>>>]><<[<+>-]<.[-]<[-]><<[
->>+<<]>>+[-<+<+>>]<[-]><<[->>+>
+<<<]>>>[-<<<+>>>]><+++><[<->-]<
[[-]<+>]<]<[-]
```

and prints...

`hiaj`
