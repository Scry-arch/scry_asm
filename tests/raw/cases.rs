use byteorder::{LittleEndian, ReadBytesExt};
use scry_isa::{AluVariant, Bits, Instruction, Instruction::*};
use scryasm::{Assemble, Raw};
use std::io::Cursor;

macro_rules! test_raw {
    (
		$name:ident [ $($asm:tt)* ] $instructions:expr
	) => {
        #[test]
        fn $name() {
            println!(stringify!($($asm)*));
            // We first assemble the string
            let assembled = Raw::assemble(std::iter::once(stringify!($($asm)*))).unwrap();

            // We then decode the assembled code to check it encodes the correct instructions
            let mut asm_reader = Cursor::new(&assembled);
            let mut decoded = Vec::new();
            while let Ok(i) = asm_reader.read_u16::<LittleEndian>() {
                decoded.push(Instruction::decode(i));
            }

            // Ensure all bytes were decoded
            // This also checks that no extra bytes were produced during the assembly
            assert_eq!(decoded.len() * 2, assembled.len());

            // Ensure they were encoding is equivalent to the expected instructions
            assert_eq!(decoded, $instructions);
        }
    };
}

test_raw! {
	independent_instructions
	[
		add =>4
		sub =>21
		echo =>100
	]
	[
		Alu(AluVariant::Add, Bits::new(4).unwrap()),
		Alu(AluVariant::Sub, Bits::new(21).unwrap()),
		EchoLong(Bits::new(100).unwrap()),
	]
}

test_raw! {
	output_to_next
	[
				inc =>instr2
				inc =>instr2
	 instr2:	dup =>3, =>16
	]
	[
		Alu(AluVariant::Inc, Bits::new(1).unwrap()),
		Alu(AluVariant::Inc, Bits::new(0).unwrap()),
		Duplicate(false, Bits::new(3).unwrap(), Bits::new(16).unwrap()),
	]
}

test_raw! {
	multiple_label_targets
	[
				inc =>instr
	 instr:     inc =>instr2
				inc =>instr2
	 instr2:    inc =>instr3
				inc =>instr3
				inc =>instr3
				inc =>instr3
	 instr3:    inc =>28
	]
	[
		Alu(AluVariant::Inc, Bits::new(0).unwrap()),
		Alu(AluVariant::Inc, Bits::new(1).unwrap()),
		Alu(AluVariant::Inc, Bits::new(0).unwrap()),
		Alu(AluVariant::Inc, Bits::new(3).unwrap()),
		Alu(AluVariant::Inc, Bits::new(2).unwrap()),
		Alu(AluVariant::Inc, Bits::new(1).unwrap()),
		Alu(AluVariant::Inc, Bits::new(0).unwrap()),
		Alu(AluVariant::Inc, Bits::new(28).unwrap()),
	]
}

test_raw! {
	output_through_jump
	[
				inc =>jmpAt=>jmpTo
				jmp jmpAt, jmpTo
	 jmpAt:     add =>0
	 jmpTo:     sub =>12
	]
	[
		Alu(AluVariant::Inc, Bits::new(1).unwrap()),
		Jump(Bits::new(0).unwrap(),Bits::new(1).unwrap()),
		Alu(AluVariant::Add, Bits::new(0).unwrap()),
		Alu(AluVariant::Sub, Bits::new(12).unwrap()),
	]
}
