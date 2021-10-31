use quickcheck::TestResult;
use quickcheck_macros::quickcheck;
use scry_isa::{Instruction, Parser};
use scryasm::{Assemble, Raw};

/// Tests that extra whitespaces can be present in the assembly without issue
#[quickcheck]
fn ignores_whitespace(
	instructions: Vec<Instruction>,
	// Adding whitespace. 0: where, 1: what type
	ws_add: Vec<(usize, u8)>,
) -> TestResult
{
	if instructions.len() == 0 {
		return TestResult::discard();
	}
	
	// First construct a single string containing all the instructions
	let mut asm = String::new();
	for instr in instructions.into_iter() {
		Instruction::print(&instr, &mut asm).unwrap();
		asm.push(' ');
	}
	
	// Then split on all possible separators
	let mut instr_tokens = vec![asm.as_str()];
	for separator in [" ", ",", "+", "=>"] {
		let mut next_toks = Vec::new();
		for tok in instr_tokens.into_iter(){
			let mut split = tok.split(separator).peekable();
			while let Some(tok) = split.next() {
				next_toks.push(tok);
				// keep the separator in its own token
				if let Some(_) = split.peek() {
					next_toks.push(separator);
				}
			}
		}
		instr_tokens = next_toks;
	}
	
	// Add whitespaces
	for (idx, ty) in ws_add.into_iter() {
		let idx = idx % instr_tokens.len();
		let ty = ty % 4;
		instr_tokens.insert(idx, match ty {
			0 => &" ",
			1 => &"\t",
			2 => &"\n",
			3 => &"\r",
			_ => unreachable!()
		})
	}
	
	// Now put it all back into one string for the test
	let mut final_asm = String::new();
	instr_tokens.into_iter().for_each(|tok| final_asm.push_str(tok));
	
	TestResult::from_bool(
		Raw::assemble(std::iter::once(final_asm.as_str())).is_ok()
	)
}
