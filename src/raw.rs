use crate::assemble::Assemble;
use byteorder::{LittleEndian, WriteBytesExt};
use scry_isa::{Instruction, Parser};
use std::collections::HashMap;
use std::iter::Peekable;

/// An assembler/disassembler for raw assembly.
///
/// "Raw" assembly contains only instructions and nothing else.
/// For text assembly, this includes label declarations and uses but nothing else.
/// For machine code, only instructions can be present.
pub struct Raw {}

#[derive(Clone)]
struct GroupIter<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool> {
    iter: Peekable<I>,
}
impl<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool> Iterator
    for GroupIter<'a, I, EMIT_LABEL>
{
    type Item = Group<'a, I, EMIT_LABEL>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_) = self.iter.peek() {
            let group_iter = Some(Group {
                done: false,
                iter: self.iter.clone(),
            });

            // Skip all items until we see ":"
            while let Some(next) = self.iter.next() {
                if next.ends_with(":") {
                    break;
                }
            }

            group_iter
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct Group<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool> {
    done: bool,
    iter: Peekable<I>,
}
impl<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool> Iterator
    for Group<'a, I, EMIT_LABEL>
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.done {
            if let Some(item) = self.iter.next() {
                return if let Some(idx) = item.find(":") {
                    self.done = true;
                    Some(item.split_at(idx).0)
                        // Only output the label if instructed
                        .filter(|_| EMIT_LABEL)
                } else {
                    Some(item)
                };
            }
        }
        None
    }
}

impl Assemble for Raw {
    type Error = String;

    fn assemble<'a, I>(asm: I) -> Result<Vec<u8>, Self::Error>
    where
        I: Iterator<Item = &'a str> + Clone,
    {
        // We first remove any whitespaces
        let cleaned = asm
            .flat_map(|s| s.split(char::is_whitespace))
            .filter(|s| !s.is_empty())
            // We split all tokens at ":", but keeping them so we can recognize the end of a group
            .flat_map(|s| s.split_inclusive(":"))
            .peekable();

        let groups = GroupIter::<_, true> {
            iter: cleaned.clone().peekable(),
        };
        let mut label_addresses: HashMap<&'a str, i32> = HashMap::new();
        let mut instr_count = 0;

        // First pass, record label addresses
        for mut group in groups {
            // First, decode as many instructions as possible
            let f = |_: Option<&str>, _: &str| 0;
            let mut next_whole = group.next();
            // how many chars from the first token that have already been consumed
            let mut sub_consumed = 0;
            while let Ok((_, tokens, chars)) = Instruction::parse(
                next_whole
                    .map(|s| s.get(sub_consumed..).unwrap())
                    .into_iter()
                    .chain(group.clone()),
                f,
            ) {
                instr_count += 1;
                if tokens > 0 || chars == next_whole.as_ref().unwrap().len(){
                    sub_consumed = 0;
                    next_whole = group.nth(tokens);
                } else {
                    sub_consumed += chars;
                }
            }

            // Then, there should be at most 1 token left, which must be a label
            if let Some(label) = next_whole
                .map(|s| s.get(sub_consumed..).unwrap())
                .filter(|s| !s.is_empty())
            {
                if let Some(_) = label_addresses.insert(label, instr_count) {
                    let mut msg = "'".to_string();
                    msg.push_str(label);
                    msg.push_str("' defined twice");
                    return Err(msg);
                }
            }

            // If any tokens are left, something must have gone wrong
            if let Some(token) = group.next() {
                let mut msg = "Phase 1 error at '".to_string();
                msg.push_str(token);
                msg.push_str("'");
                return Err(msg);
            }
        }

        // Second pass, final assembly
        let groups = GroupIter::<_, false> {
            iter: cleaned.clone().peekable(),
        };
        let mut result = Vec::with_capacity(instr_count as usize);
        let mut instr_count = 0;
        for mut group in groups {
            let mut first = group.next();
            // how many chars from the first token that have already been consumed
            let mut sub_consumed = 0;
            while let Ok((instr, tokens, chars)) = Instruction::parse(
                first
                    .map(|s| s.get(sub_consumed..).unwrap())
                    .into_iter()
                    .chain(group.clone()),
                |from: Option<&str>, to: &str| {
                    2 * (
                        label_addresses[to] - from.map_or(instr_count, |from| label_addresses[from])
                    )
                },
            ) {
                result.write_u16::<LittleEndian>(instr.encode()).unwrap();
                instr_count += 1;
                if tokens > 0 {
                    sub_consumed = 0;
                    first = group.nth(tokens);
                } else {
                    sub_consumed += chars;
                }
            }
        }
        Ok(result)
    }
}
