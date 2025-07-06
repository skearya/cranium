fn main() {
    let src = "> +++ [-<+>] char a = 3;

< [->+>+<<] >> [-<<+>>] push(a) < check [ [-] clear cond    {while(a) boilerplate}
    < [->+>+<<] >> [-<<+>>] push(a) +> push(1)              {a = sub(a 1)}
    <[-<->] sub() << [-] > [-<+>] mov(a)
    < [->+>+<<] >> [-<<+>>] push(a) < check                 {more boilerplate}
]";

    let mut out = String::new();

    let mut depth = 0;

    for (index, c) in src.char_indices() {
        match c {
            '[' => {
                out.push('\n');

                (0..depth).for_each(|_| out.push('\t'));
                out.push('[');
                depth += 1;

                out.push('\n');

                (0..depth).for_each(|_| out.push('\t'));
            }
            ']' => {
                out.push('\n');

                depth -= 1;
                (0..depth).for_each(|_| out.push('\t'));
                out.push(']');

                out.push('\n');

                (0..depth).for_each(|_| out.push('\t'));
            }
            '>' | '<' | '+' | '-' | '.' | ',' => out.push(c),
            _ => {}
        }
    }

    println!("{out}");
}
