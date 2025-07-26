use crate::assemble::Assemble;
use byteorder::{LittleEndian, WriteBytesExt};
use regex::Regex;
use scry_isa::{
	Arrow, CanConsume, Comma, Instruction, Keyword, Maybe, ParseError, ParseErrorType, Parser,
	Resolve, Symbol, Then, Type, TypeMatcher,
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
	F: Fn(Resolve<'a>) -> Result<i32, &'a str>,
{
	let f: &F = f.borrow();
	Then::<DirBytesKeyword, Then<TypeMatcher<4, 3>, Comma>>::parse::<_, F, _>(iter.clone(), f)
		.or(Err("Not '.bytes' directive".to_owned()))
		.and_then(|((_, (typ_bits, _)), consumed)| {
			let typ: Type = typ_bits.try_into().unwrap();
			let signed = typ.is_signed_int();
			let pow2 = typ.size_pow2();

			assert!(pow2 <= 4, "We don't support values of more than 128 bits");
			let (consumed, next_token) = consumed.advance_iter_in_place(&mut iter);

			let parsed_ref = Then::<Symbol, Maybe<Then<Arrow, Symbol>>>::parse::<_, F, _>(
				next_token.clone().into_iter().chain(iter.clone()),
				f,
			)
			.and_then(|((sym1, sym2), consumed2)| {
				if let Some((_, sym2)) = sym2
				{
					f(Resolve::Distance(sym1, sym2))
				}
				else
				{
					f(Resolve::Address(sym1))
				}
				.map_err(|_| {
					ParseError::from_consumed(consumed2.clone(), ParseErrorType::UnknownSymbol)
				})
				.map(|addr| (addr, consumed2))
			});

			let size = typ.size() as u32;
			if signed
			{
				parsed_ref
					.map(|(val, consumed)| (val as i128, consumed))
					.or_else(|_| {
						<i128 as Parser>::parse::<_, F, _>(
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
						<u128 as Parser>::parse::<_, F, _>(
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

		let mut clean_peek = cleaned.clone().peekable();
		let mut label_addresses: HashMap<&'a str, i32> = HashMap::new();
		let mut byte_count = 0;

		let mnems_pat = scry_isa::INSTRUCTION_MNEMONICS.iter()
			.map(|d| regex::escape(d)) // ensures special characters are treated literally
			.collect::<Vec<String>>()
			.join("|");
		let dirs_pat = [DirBytesKeyword::WORD].iter()
			.map(|d| regex::escape(d)) // ensures special characters are treated literally
			.collect::<Vec<String>>()
			.join("|");

		let re_mnems = Regex::new(&format!("^({})$", mnems_pat)).unwrap();
		let re_dirs = Regex::new(&format!("^({})$", dirs_pat)).unwrap();

		// First pass, record label addresses
		loop
		{
			let tok = if let Some(tok) = clean_peek.next()
			{
				tok
			}
			else
			{
				// done
				break;
			};

			if tok.ends_with(':') || clean_peek.peek() == Some(&":")
			{
				// Found the label

				let label = tok.split(':').next().unwrap();
				if let Some(_) = label_addresses.insert(label, byte_count)
				{
					let mut msg = "'".to_string();
					msg.push_str(label);
					msg.push_str("' defined twice");
					return Err(msg);
				}
				continue;
			}

			if re_dirs.is_match(tok)
			{
				// parse directive

				match parse_bytes_direcive(
					Some(tok).into_iter().chain(clean_peek.clone()),
					|_: Resolve| Ok(2),
				)
				{
					Ok((bytes, consumed)) =>
					{
						byte_count += bytes.len() as i32;
						consumed
							.advance_iter_in_place(
								&mut Some(tok).into_iter().chain(&mut clean_peek),
							)
							.1;
						continue;
					},
					Err(err) =>
					{
						let mut msg = "Directive parsing error: ".to_string();
						msg.push_str(err.as_str());
						return Err(msg);
					},
				}
			}
			else if re_mnems.is_match(tok)
			{
				// Start of instruction, count up 2 bytes
				byte_count += 2;
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
				let f = |resolve| {
					match resolve
					{
						Resolve::Address(sym) => label_addresses.get(sym).cloned().ok_or(sym),
						Resolve::DistanceCurrent(sym) =>
						{
							label_addresses
								.get(sym)
								.ok_or(sym)
								.map(|addr| addr - byte_count)
						},
						Resolve::Distance(sym1, sym2) =>
						{
							if !label_addresses.contains_key(sym2)
							{
								Err(sym2)
							}
							else if !label_addresses.contains_key(sym1)
							{
								Err(sym1)
							}
							else
							{
								Ok(label_addresses[sym2] - label_addresses[sym1])
							}
						},
					}
				};

				// Try to parse a directive
				let all_tokens = next_token.clone().into_iter().chain(group.clone());
				if let Ok((bytes, consumed)) = parse_bytes_direcive(all_tokens.clone(), f)
				{
					byte_count += bytes.len() as i32;
					result.extend(bytes.into_iter());
					next_token = consumed
						.advance_iter_in_place(&mut next_token.into_iter().chain(&mut group))
						.1;
					continue;
				}

				// Try to parse an instruction
				match Instruction::parse(all_tokens.clone(), f)
				{
					Ok((instr, consumed)) =>
					{
						result.write_u16::<LittleEndian>(instr.encode()).unwrap();
						byte_count += 2;
						next_token = consumed
							.advance_iter_in_place(&mut next_token.into_iter().chain(&mut group))
							.1;
						continue;
					},
					Err(err) =>
					{
						match err.err_type
						{
							ParseErrorType::UnknownSymbol =>
							{
								return Err(format!(
									"Unknown label: {}",
									err.extract_from_iter(all_tokens)
								))
							},
							ParseErrorType::OutOfBoundValue(val, min, max) =>
							{
								return Err(format!(
									"Invalid Value (Should be {} - {}): {}\nSource: {}",
									min,
									max,
									val,
									err.extract_from_iter(all_tokens)
								))
							},
							// Group finished
							_ => break,
						}
					},
				}
			}
		}
		Ok(result)
	}
}
