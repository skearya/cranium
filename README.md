# cranium

C compiler targeting [brainfuck](https://brainfuck.org/), an 8-instruction language faithful to the original Turing Machine.

Written in Rust, this program utilizes [tree-sitter](https://github.com/tree-sitter/tree-sitter) and [tree_sitter_c](https://docs.rs/tree-sitter-c/latest/tree_sitter_c/) for parsing.

Currently supports a `main` function definition, a `putchar` call, the `char` and `bool` types, stack variables, if-else statements, for statements, while statements, and several expressions.

This repository includes an in-house brainfuck interpreter with debug capabilities as well as a complete strictly typed wrapper around the untyped tree-sitter Rust API.

## Example

```c
int main() {
  for (char i = 0; i != 3; i++) {
    if (i == 2) {
      putchar('a');
    }
    putchar('h' + i);
  }
}
```

becomes...

```bf
>><[<<+>>-]><<[->>+>
+<<<]>>>[-<<<+>>>]><
+++><[<->-]<[[-]<+>]
<[[-]+><<[->>+>+<<<]
>>>[-<<<+>>>]><++><[
-<->]<[[-]<->]<[[-]+
++++++++++++++++++++
++++++++++++++++++++
++++++++++++++++++++
++++++++++++++++++++
++++++++++++++++><.[
-]]+++++++++++++++++
++++++++++++++++++++
++++++++++++++++++++
++++++++++++++++++++
++++++++++++++++++++
+++++++><<[->>+>+<<<
]>>>[-<<<+>>>]><<[<+
>-]<.[-]><<[->>+<<]>
>+[-<+<+>>]<[-]><<[-
>>+>+<<<]>>>[-<<<+>>
>]><+++><[<->-]<[[-]
<+>]<]<[-]
```

and prints...

`hiaj`
