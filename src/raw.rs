use crate::assemble::Assemble;
use byteorder::{LittleEndian, WriteBytesExt};
use scry_isa::{
	Arrow, CanConsume, Comma, Instruction, IntSize, Keyword, Maybe, Parser, Resolve, Symbol, Then,
};
use std::{borrow::Borrow, collections::HashMap, iter::Peekable};

/// An assembler/disassembler for raw assembly.
///
/// "Raw" assembly contains only instructions and nothing else.
/// For text assembly, this includes label declarations and uses but nothing
/// else. For machine code, only instructions can be present.
pub struct Raw {}

#[derive(Clone)]
struct GroupIter<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool>
{
	iter: Peekable<I>,
}
impl<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool> Iterator
	for GroupIter<'a, I, EMIT_LABEL>
{
	type Item = Group<'a, I, EMIT_LABEL>;

	fn next(&mut self) -> Option<Self::Item>
	{
		if let Some(_) = self.iter.peek()
		{
			let group_iter = Some(Group {
				done: false,
				iter: self.iter.clone(),
			});

			// Skip all items until we see ":"
			while let Some(next) = self.iter.next()
			{
				if next.ends_with(":")
				{
					break;
				}
			}

			group_iter
		}
		else
		{
			None
		}
	}
}

#[derive(Clone, Debug)]
struct Group<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool>
{
	done: bool,
	iter: Peekable<I>,
}
impl<'a, I: Clone + Iterator<Item = &'a str>, const EMIT_LABEL: bool> Iterator
	for Group<'a, I, EMIT_LABEL>
{
	type Item = I::Item;

	fn next(&mut self) -> Option<Self::Item>
	{
		if !self.done
		{
			if let Some(item) = self.iter.next()
			{
				return if let Some(idx) = item.find(":")
				{
					self.done = true;
					Some(item.split_at(idx).0)
                        // Only output the label if instructed
                        .filter(|_| EMIT_LABEL)
                        // Don't output empty string
                        .filter(|s| !s.is_empty())
				}
				else
				{
					Some(item)
				};
			}
		}
		None
	}
}

struct DirBytesKeyword();
impl Keyword for DirBytesKeyword
{
	const WORD: &'static str = ".bytes";
}

fn parse_bytes_direcive<'a, F, B>(
	mut iter: impl Iterator<Item = &'a str> + Clone,
	f: B,
) -> Result<(Vec<u8>, CanConsume), String>
where
	B: Borrow<F>,
	F: Fn(Resolve) -> i32,
{
	Then::<DirBytesKeyword, Then<IntSize, Comma>>::parse::<_, F, _>(iter.clone(), f.borrow())
		.or(Err("Not '.bytes' directive".to_owned()))
		.and_then(|((_, ((signed, pow2), _)), consumed)| {
			assert!(
				pow2.value <= 4,
				"We don't support values of more than 128 bits"
			);
			let (consumed, next_token) = consumed.advance_iter_in_place(&mut iter);

			let parsed_ref = Then::<Symbol, Maybe<Then<Arrow, Symbol>>>::parse::<_, F, _>(
				next_token.clone().into_iter().chain(iter.clone()),
				f.borrow(),
			)
			.and_then(|((sym1, sym2), consumed2)| {
				if let Some((_, sym2)) = sym2
				{
					Ok((f.borrow()(Resolve::Distance(sym1, sym2)), consumed2))
				}
				else
				{
					Ok((f.borrow()(Resolve::Address(sym1)), consumed2))
				}
			});

			let size = 2u32.pow(pow2.value as u32);
			if signed
			{
				parsed_ref
					.map(|(val, consumed)| (val as i128, consumed))
					.or_else(|_| {
						<i128 as Parser>::parse(
							next_token.clone().into_iter().chain(iter.clone()),
							f,
						)
					})
					.map_err(|err| format!("{:?}", err))
					.and_then(|(val, consumed2)| {
						let min_value = (2i128.pow((size * 8) - 1) * (-1)) - 1;
						let max_value = 2i128.pow((size * 8) - 1);

						if min_value <= val && max_value >= val
						{
							Ok((
								val.to_le_bytes().into_iter().take(size as usize).collect(),
								consumed.then(&consumed2),
							))
						}
						else
						{
							Err(format!(
								"Bytes value out of bounds (actual, minimum, maximum): {}, {}, {}",
								val, min_value, max_value
							))
						}
					})
			}
			else
			{
				parsed_ref
					.map(|(val, consumed)| (val as u128, consumed))
					.or_else(|_| {
						<u128 as Parser>::parse(
							next_token.clone().into_iter().chain(iter.clone()),
							f,
						)
					})
					.map_err(|err| format!("{:?}", err))
					.and_then(|(val, consumed2)| {
						let max_value = 2u128.pow(size * 8);

						if max_value >= val
						{
							Ok((
								val.to_le_bytes().into_iter().take(size as usize).collect(),
								consumed.then(&consumed2),
							))
						}
						else
						{
							Err(format!(
								"Bytes value out of bounds (actual, minimum, maximum): {}, {}, {}",
								val, 0, max_value
							))
						}
					})
			}
		})
}

impl Assemble for Raw
{
	type Error = String;

	fn assemble<'a, I>(asm: I) -> Result<Vec<u8>, Self::Error>
	where
		I: Iterator<Item = &'a str> + Clone,
	{
		let cleaned = asm
            // Remove comments
            .flat_map(|mut s| {
                let mut result = Vec::new();
                while let Some((before, after)) = s.split_once(';') {
                    // Keep anything before comment
                    result.push(before);
                    // Now check anything after first newline
                    s = after
                        .split_once(&['\r', '\n'])
                        .map_or("", |(_, after_newline)| after_newline);
                }
                // The remaining cannot have comments
                result.push(s);
                result.into_iter()
            })
            // Remove whitespace
            .flat_map(|s| s.split(char::is_whitespace))
            .filter(|s| !s.is_empty())
            // We split all tokens after ":", so we can recognize the end of a group
            .flat_map(|s| s.split_inclusive(":"))
            .peekable();

		let groups = GroupIter::<_, true> {
			iter: cleaned.clone().peekable(),
		};
		let mut label_addresses: HashMap<&'a str, i32> = HashMap::new();
		let mut byte_count = 0;

		// First pass, record label addresses
		for mut group in groups
		{
			// First, decode as many instructions as possible using dummy address
			let f = |_: Resolve| 2;
			let mut next_token = None;

			loop
			{
				// Try to parse a directive
				if let Ok((bytes, consumed)) =
					parse_bytes_direcive(next_token.clone().into_iter().chain(group.clone()), f)
				{
					byte_count += bytes.len() as i32;
					next_token = consumed
						.advance_iter_in_place(&mut next_token.into_iter().chain(&mut group))
						.1;
					continue;
				}

				// Try to parse an instruction
				if let Ok((_instr, consumed)) =
					Instruction::parse(next_token.clone().into_iter().chain(group.clone()), f)
				{
					byte_count += 2;
					next_token = consumed
						.advance_iter_in_place(&mut next_token.into_iter().chain(&mut group))
						.1;
					continue;
				}
				break;
			}

			// Then, there should be at most 1 token left, which must be a label
			if let Some(label) = next_token.into_iter().chain(&mut group).next()
			{
				if let Some(_) = label_addresses.insert(label, byte_count)
				{
					let mut msg = "'".to_string();
					msg.push_str(label);
					msg.push_str("' defined twice");
					return Err(msg);
				}
			}

			// If any tokens are left, something must have gone wrong
			if let Some(token) = next_token.into_iter().chain(&mut group).next()
			{
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
		let mut result = Vec::with_capacity(byte_count as usize);
		let mut byte_count = 0;
		for mut group in groups
		{
			let mut next_token = None;

			loop
			{
				let f = |resolve: Resolve| {
					match resolve
					{
						Resolve::Address(sym) => label_addresses[sym],
						Resolve::DistanceCurrent(sym) => label_addresses[sym] - byte_count,
						Resolve::Distance(sym1, sym2) =>
						{
							label_addresses[sym2] - label_addresses[sym1]
						},
					}
				};

				// Try to parse a directive
				if let Ok((bytes, consumed)) =
					parse_bytes_direcive(next_token.clone().into_iter().chain(group.clone()), f)
				{
					byte_count += bytes.len() as i32;
					result.extend(bytes.into_iter());
					next_token = consumed
						.advance_iter_in_place(&mut next_token.into_iter().chain(&mut group))
						.1;
					continue;
				}

				// Try to parse an instruction
				if let Ok((instr, consumed)) =
					Instruction::parse(next_token.clone().into_iter().chain(group.clone()), f)
				{
					result.write_u16::<LittleEndian>(instr.encode()).unwrap();
					byte_count += 2;
					next_token = consumed
						.advance_iter_in_place(&mut next_token.into_iter().chain(&mut group))
						.1;
					continue;
				}
				break;
			}
		}
		Ok(result)
	}
}
