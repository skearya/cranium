use std::io::{Read, stdin};

#[derive(Debug)]
pub enum Token {
    IncPtr,
    DecPtr,
    IncVal,
    DecVal,
    PutChar,
    GetChar,
    Loop(Vec<Token>),
    Debug,
}

fn tokenize(chars: &mut impl Iterator<Item = char>) -> Vec<Token> {
    let mut tokens = vec![];

    while let Some(char) = chars.next() {
        tokens.push(match char {
            '>' => Token::IncPtr,
            '<' => Token::DecPtr,
            '+' => Token::IncVal,
            '-' => Token::DecVal,
            '.' => Token::PutChar,
            ',' => Token::GetChar,
            '[' => Token::Loop(tokenize(chars)),
            ']' => return tokens,
            '@' => Token::Debug,
            char if char.is_whitespace() => continue,
            invalid => panic!("invalid instruction: {invalid}"),
        });
    }

    tokens
}

pub fn interpret(tokens: &[Token], memory: &mut [u8], ptr: &mut usize) {
    for token in tokens {
        match token {
            Token::IncPtr => *ptr += 1,
            Token::DecPtr => *ptr -= 1,
            Token::IncVal => memory[*ptr] = memory[*ptr].wrapping_add(1),
            Token::DecVal => memory[*ptr] = memory[*ptr].wrapping_sub(1),
            Token::PutChar => print!("{}", char::from(memory[*ptr])),
            Token::GetChar => {
                let mut buffer = [0; 1];
                stdin().read_exact(&mut buffer).unwrap();
                memory[*ptr] = buffer[0];
            }
            Token::Loop(tokens) => {
                while memory[*ptr] != 0 {
                    interpret(tokens, memory, ptr);
                }
            }
            Token::Debug => print(memory, *ptr),
        }
    }
}

pub fn run(src: &str) {
    let tokens = tokenize(&mut src.chars());

    let mut memory = Box::new([0; 30_000]);
    let mut ptr = 0;

    interpret(&tokens, &mut memory[..], &mut ptr);

    print(&memory[..], ptr);
}

fn print(memory: &[u8], ptr: usize) {
    const WIDTH: usize = 24;

    const BRIGHT_CYAN: &str = "\x1b[96m";
    const BRIGHT_MAGENTA: &str = "\x1b[95m";
    const END: &str = "\x1b[0m";

    println!();

    // Top
    print!("╭");
    for _ in 0..WIDTH * 3 {
        print!("─");
    }
    println!("╮");

    // Memory
    print!("│ ");
    for (index, data) in memory[..WIDTH].iter().enumerate() {
        print!("{BRIGHT_CYAN}{}{END}", data);

        if index != WIDTH - 1 {
            print!(", ");
        }
    }
    println!(" │");

    // Cursor
    print!("│ ");
    for index in 0..WIDTH {
        if index == ptr {
            print!("{BRIGHT_MAGENTA}^{END}");
        } else {
            print!(" ");
        }

        if index != WIDTH - 1 {
            print!("  ")
        }
    }
    println!(" │");

    // Bottom
    print!("╰");
    for _ in 0..WIDTH * 3 {
        print!("─");
    }
    println!("╯");
}
