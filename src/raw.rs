use crate::assemble::Assemble;
use scry_isa::{
	Instruction, Parser,
};
use byteorder::{LittleEndian, WriteBytesExt};

/// An assembler/disassembler for raw assembly.
///
/// "Raw" assembly contains only instructions and nothing else.
/// For text assembly, this includes label declarations and uses but nothing else.
/// For machine code, only instructions can be present.
pub struct Raw{

}

#[derive(Clone)]
struct SplitColon<'a, I: Clone + Iterator<Item=&'a str>> {
	iter: I
}

impl<'a, I: Clone + Iterator<Item=&'a str>> Iterator for SplitColon<'a, I>
{
	type Item = Group<'a,I>;
	
	fn next(&mut self) -> Option<Self::Item> {
		if let Some(item) = self.iter.next() {
			Some(Group{next_item: Some(item), iter: self.iter.clone()})
		} else{
			None
		}
	}
}

#[derive(Clone)]
struct Group<'a, I: Clone + Iterator<Item=&'a str>> {
	next_item: Option<I::Item>,
	iter: I,
}

impl<'a, I: Clone + Iterator<Item=&'a str>> Iterator for Group<'a, I>
{
	type Item = I::Item;
	
	fn next(&mut self) -> Option<Self::Item> {
		if let Some(item) = self.next_item {
			self.next_item = self.iter.next().filter(|&i| i != ":");
			Some(item)
		} else {
			None
		}
	}
}

impl Assemble for Raw {
	type Error = ();
	
	fn assemble<'a, I>(asm: I) -> Result<Vec<u8>, Self::Error> where I: Iterator<Item=&'a str> + Clone {
		// We first remove any whitespaces
		let mut cleaned = asm.flat_map(|s| s.split(char::is_whitespace)).filter(|s| !s.is_empty());
			
		let mut result = Vec::new();
		let f = |_:Option<&str>, _:&str | 0;
		let mut first = cleaned.next();
		// how many chars from the first token that have already been consumed
		let mut sub_consumed = 0;
		while let Ok((instr, tokens, chars)) = Instruction::parse(
			first.map(|s| s.get(sub_consumed..).unwrap()).into_iter().chain(cleaned.clone()), f
		)
		{
			result.write_u16::<LittleEndian>(instr.encode()).unwrap();
			
			if tokens > 0 {
				sub_consumed = 0;
				first = cleaned.nth(tokens);
			}
			else {
				sub_consumed += chars;
			}
		}
		Ok(result)
	}
}