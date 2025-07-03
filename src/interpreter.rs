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
            Token::IncVal => memory[*ptr] += 1,
            Token::DecVal => memory[*ptr] -= 1,
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
        }
    }
}

pub fn run(src: &str) {
    let tokens = tokenize(&mut src.chars());

    let mut memory = Box::new([0; 30000]);
    let mut index = 0;

    interpret(&tokens, &mut memory[..], &mut index);

    dbg!(&memory[..16]);
    dbg!(index);
}
