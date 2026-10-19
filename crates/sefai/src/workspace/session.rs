use super::{schema, schema::invalid};
use sefai_core::{Message, Role};
use std::{
    collections::VecDeque,
    io::{self, BufRead, Read},
};

pub const SESSION_BYTES_MAX: u64 = 64 * 1024 * 1024;
pub const TAIL_MESSAGES_MAX: usize = 128;
const RECORD_COUNT_MAX: usize = 65536;

/// Reads the whole grammar but retains only the bounded tail in memory.
/// Quoted body lines cannot become roles, headings or record delimiters.
pub fn read(input: impl BufRead, limit: usize) -> io::Result<(schema::Session, Vec<Message>)> {
    if !(1..=TAIL_MESSAGES_MAX).contains(&limit) {
        return Err(invalid("session.limit: expected 1..128 messages"));
    }
    let mut input = input.take(SESSION_BYTES_MAX + 1);
    let mut front = String::new();
    let first = line(&mut input)?.ok_or_else(|| invalid("session: missing front matter"))?;
    if first != "---" {
        return Err(invalid("session: expected opening ---"));
    }
    front.push_str("---\n");
    loop {
        let value = line(&mut input)?.ok_or_else(|| invalid("session: truncated front matter"))?;
        if front.len() + value.len() > schema::FRONT_BYTES_MAX {
            return Err(invalid("session: front matter exceeds 16384 bytes"));
        }
        front.push_str(&value);
        front.push('\n');
        if value == "---" {
            break;
        }
    }
    let (header, _) = schema::parse::<schema::Session>(&front)?;
    schema::version(header.schema)?;
    schema::id(&header.id)?;
    let mut messages: VecDeque<Message> = VecDeque::new();
    let mut bytes = 0;
    let mut count = 0;
    while let Some(heading) = line(&mut input)? {
        if heading.is_empty() {
            continue;
        }
        count += 1;
        if count > RECORD_COUNT_MAX {
            return Err(invalid("session: exceeds 65536 records"));
        }
        let role = match heading.as_str() {
            "## user" => Role::User,
            "## assistant" => Role::Assistant,
            _ => return Err(invalid("session.role: expected ## user or ## assistant")),
        };
        let mut body = String::new();
        let mut lines = 0;
        loop {
            let value = line(&mut input)?
                .ok_or_else(|| invalid("session: interrupted record; missing <!-- end -->"))?;
            if value == "<!-- end -->" {
                break;
            }
            let value = value
                .strip_prefix("> ")
                .ok_or_else(|| invalid("session.body: expected > quoted text"))?;
            if body.len() + value.len() + usize::from(lines > 0) > schema::FILE_BYTES_MAX {
                return Err(invalid("session.body: exceeds 1048576 bytes"));
            }
            if lines > 0 {
                body.push('\n');
            }
            body.push_str(value);
            lines += 1;
        }
        if lines == 0 {
            return Err(invalid("session.body: expected at least one quoted line"));
        }
        bytes += body.len();
        messages.push_back(Message {
            role,
            content: body,
        });
        while messages.len() > limit || bytes > schema::FILE_BYTES_MAX {
            if let Some(message) = messages.pop_front() {
                bytes -= message.content.len();
            }
        }
    }
    if input.limit() == 0 {
        return Err(invalid("session: exceeds 67108864 bytes"));
    }
    Ok((header, messages.into()))
}

pub fn encode(role: &str, text: &str) -> io::Result<String> {
    if !matches!(role, "user" | "assistant") {
        return Err(invalid("session.role: expected user or assistant"));
    }
    if text.len() > schema::FILE_BYTES_MAX || text.contains('\0') {
        return Err(invalid(
            "session.body: exceeds 1048576 bytes or contains nul",
        ));
    }
    let text = text.replace("\r\n", "\n");
    if text.contains('\r') {
        return Err(invalid(
            "session.body: bare carriage returns are unsupported",
        ));
    }
    let mut result = format!("## {role}\n");
    for line in text.split('\n') {
        result.push_str("> ");
        result.push_str(line);
        result.push('\n');
    }
    result.push_str("<!-- end -->\n");
    Ok(result)
}

fn line(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut result = String::new();
    let length = (&mut *input)
        .take(schema::FILE_BYTES_MAX as u64 + 4)
        .read_line(&mut result)?;
    if length > schema::FILE_BYTES_MAX + 3 {
        return Err(invalid("session.line: exceeds 1048576 bytes"));
    }
    if length == 0 {
        return Ok(None);
    }
    if !result.ends_with('\n') {
        return Err(invalid("session: truncated line"));
    }
    result.pop();
    if result.ends_with('\r') {
        result.pop();
    }
    Ok(Some(result))
}
