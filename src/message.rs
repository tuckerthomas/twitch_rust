use nom::{
    *,
    number::complete::recognize_float
};

named!(colon<&str, &str>,
    take_while1!(|c| c == ':')
);

named!(space<&str, &str>,
    take_while1!(|c| c == ' ')
);

named!(crlf<&str, &str>,
    tag!("\r\n")
);

#[derive(Debug)]
pub struct ChatPrefix<'b> {
    nick: &'b str,
    user: &'b str,
    host: &'b str
}

#[derive(Debug)]
pub enum Prefix<'a> {
    Server(&'a str),
    Chat(ChatPrefix<'a>)
}

named!(server_prefix<&str, Prefix>,
    do_parse!(
        server_name: take_until1!(" ") >>
        (Prefix::Server(server_name))
    )
);

named!(chat_prefix<&str, Prefix>,
    do_parse!(
        nick: take_until1!("!") >>
        user: take_until1!("@") >>
        host: take_until1!(" ") >>
        (Prefix::Chat(
            ChatPrefix{
                nick: nick, 
                user: user, 
                host: host
            }
        ))
    )
);

named!(prefix<&str, Prefix>,
    alt!(server_prefix | chat_prefix)
);

named!(middle<&str, &str>,
    take_while1!(|c| c != ' ' || c != '\r' || c != '\n')

);

named!(trailing<&str, &str>,
    take_while!(|c| c != '\r' || c != '\n')
);

named!(params<&str, &str>,
    do_parse!(
        space >>
        alt!(
            pair!(colon, trailing) | pair!(middle, params)
        ) >>
        ("yeet")
    )
);

#[derive(Debug)]
enum Command<'a> {
    Numbers(f32),
    Letters(&'a str)
}

named!(command_letters<&str, Command>,
    do_parse!(
        letters: take_while1!(char::is_alphabetic) >>
        (Command::Letters(letters))
    )
);

named!(command_numbers<&str, Command>,
    do_parse!(
        numbers: flat_map!(recognize_float, parse_to!(f32)) >>
        (Command::Numbers(numbers))
    )
);

named!(command<&str, Command>,
    alt!(command_letters | command_numbers)
);

#[derive(Debug)]
pub struct Message<'a> {
    prefix: Prefix<'a>,
    command: Command<'a>,
    params: &'a str

}

trace_macros!(true);
named!(message_parse<&str, Message>,
    do_parse!(
        colon >>
        prefix: prefix >>
        space >>
        command: command >>
        params: params >>
        crlf >>
        (Message{prefix: prefix, command: command, params: params})
    )
);

pub fn parse_message(message_string: &str) -> Message {
    trace_macros!(true);
    let (what, message) = message_parse(message_string).unwrap();
    trace_macros!(false);
    println!("the heck: {}", what);
    return message;
}