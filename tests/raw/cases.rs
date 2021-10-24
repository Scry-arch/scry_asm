use std::io::{Cursor};
use scry_isa::{
	Instruction, Instruction::*, AluVariant, Bits
};
use scryasm::{
	Assemble, Raw
};
use byteorder::{LittleEndian, ReadBytesExt};

macro_rules! test_raw {
	(
		$name:ident $asm:literal $instructions:expr
	) => {
		#[test]
		fn $name() {
			// We first assemble the string
			let assembled = Raw::assemble(std::iter::once($asm)).unwrap();
			
			// We then decode the assembled code to check it encodes the correct instructions
			let mut asm_reader = Cursor::new(&assembled);
			let mut decoded = Vec::new();
			while let Ok(i) = asm_reader.read_u16::<LittleEndian>() {
				decoded.push(Instruction::decode(i));
			}
			
			// Ensure all bytes were decoded
			// This also checks that no extra bytes were produced during the assembly
			assert_eq!(decoded.len()*2, assembled.len());
			
			// Ensure they were encoding is equivalent to the expected instructions
			assert_eq!(decoded, $instructions);
		}
	};
}

test_raw! {
	independent_instructions
	"add =>4 \
	 sub =>21 \
	 echo =>100 \
	"
	[
		Alu(AluVariant::Add, Bits::new(4).unwrap()),
		Alu(AluVariant::Sub, Bits::new(21).unwrap()),
		EchoLong(Bits::new(100).unwrap()),
	]
}

test_raw! {
	output_to_next
	"			inc =>instr2 	\
	 instr2:	dup =>3 =>16 	\
	"
	[
		Alu(AluVariant::Inc, Bits::new(0).unwrap()),
		Duplicate(false, Bits::new(3).unwrap(), Bits::new(16).unwrap()),
	]
}

