use byteorder::{ByteOrder, LittleEndian};
use quickcheck::{Arbitrary, Gen, TestResult};
use quickcheck_macros::quickcheck;
use scry_isa::{Instruction, Parser};
use scryasm::{Assemble, Raw};

/// Converts a list of instructions into their assembly
fn to_asm_string(instructions: &Vec<Instruction>) -> String
{
	let mut asm = String::new();
	for instr in instructions.iter()
	{
		Instruction::print(instr, &mut asm).unwrap();
		asm.push(' ');
	}
	asm
}

/// Tests that the given string is assembled into the given instructions
fn test_assemble(assembly: String, instructions: &Vec<Instruction>) -> TestResult
{
	TestResult::from_bool(
		// Assemble the string
		Raw::assemble(std::iter::once(assembly.as_str())).map_or(false, |bytes| {
			// If successfully assembled, decode it to instructions
			let mut result_instructions = Vec::new();
			for i in 0..(bytes.len() / 2)
			{
				result_instructions.push(Instruction::decode(LittleEndian::read_u16(
					&bytes.as_slice()[i * 2..],
				)))
			}
			// Then check they are as expected
			let mut equal = result_instructions.len() == instructions.len();
			for (i1, i2) in result_instructions
				.into_iter()
				.zip(instructions.into_iter())
			{
				equal &= i1 == *i2;
			}
			equal
		}),
	)
}

/// Tests can assemble instructions that don't use symbols.
#[quickcheck]
fn assemble_simple(instructions: Vec<Instruction>) -> TestResult
{
	test_assemble(to_asm_string(&instructions), &instructions)
}

/// Arbitrary ignored characters and sequences
#[derive(Clone, Debug)]
enum IgnoredChars
{
	Space,
	Tab,
	LineFeed,
	CarriageReturn,
	/// Comment as body and different types of newlines
	Comment(String, &'static str),
}
impl IgnoredChars
{
	fn to_string(&self) -> String
	{
		use IgnoredChars::*;
		match self
		{
			Space => " ".to_string(),
			Tab => "\t".to_string(),
			LineFeed => "\n".to_string(),
			CarriageReturn => "\r".to_string(),
			Comment(body, newline) => ";".to_string() + body + newline,
		}
	}
}
impl Arbitrary for IgnoredChars
{
	fn arbitrary(g: &mut Gen) -> Self
	{
		use IgnoredChars::*;
		match g
			.choose(&[
				Space,
				Tab,
				LineFeed,
				CarriageReturn,
				Comment(String::new(), ""),
			])
			.unwrap()
			.clone()
		{
			Comment(_, _) =>
			{
				let mut comment_body = String::arbitrary(g);
				// Remove any newlines
				while let Some(idx) = comment_body.find(&['\n', '\r'])
				{
					comment_body.remove(idx);
				}
				Comment(comment_body, g.choose(&["\n", "\r", "\r\n"]).unwrap())
			},
			other => other,
		}
	}

	fn shrink(&self) -> Box<dyn Iterator<Item = Self>>
	{
		use IgnoredChars::*;
		let mut result = Vec::new();
		match self
		{
			Tab | LineFeed | CarriageReturn => result.push(Space),
			Comment(body, newline) =>
			{
				if newline.len() > 1
				{
					result.push(Comment(body.clone(), "\n"));
					result.push(Comment(body.clone(), "\r"));
				}
				else if newline == &"\r"
				{
					result.push(Comment(body.clone(), "\n"));
				}
				result.extend(body.shrink().map(|mut new_body| {
					// remove any newlines
					while let Some(idx) = new_body.find(&['\n', '\r'])
					{
						new_body.remove(idx);
					}
					Comment(new_body, newline)
				}))
			},
			Space => (), // Nothing to shrink
		}
		Box::new(result.into_iter())
	}
}

/// Tests that extra whitespaces and comments can be present in the assembly
/// without issue
#[quickcheck]
fn ignored(
	instructions: Vec<Instruction>,
	// Adding whitespace. 0: where, 1: what type
	ignored: Vec<(usize, IgnoredChars)>,
) -> TestResult
{
	// First construct a single string containing all the instructions
	let asm = to_asm_string(&instructions);

	// Then split on all possible separators
	let mut instr_tokens = vec![asm];
	for separator in [" ", ",", "+", "=>"]
	{
		let mut next_toks = Vec::new();
		for tok in instr_tokens.into_iter()
		{
			let mut split = tok.split(separator).peekable();
			while let Some(tok) = split.next()
			{
				next_toks.push(tok.to_string());
				// keep the separator in its own token
				if let Some(_) = split.peek()
				{
					next_toks.push(separator.to_string());
				}
			}
		}
		instr_tokens = next_toks;
	}

	// Add ignored
	ignored
		.into_iter()
		.for_each(|(idx, ty)| instr_tokens.insert(idx % instr_tokens.len(), ty.to_string()));

	// Now put it all back into one string for the test
	let mut final_asm = String::new();
	instr_tokens
		.into_iter()
		.for_each(|tok| final_asm.push_str(tok.as_str()));

	test_assemble(final_asm, &instructions)
}
