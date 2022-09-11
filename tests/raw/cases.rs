use byteorder::{LittleEndian, ReadBytesExt};
use scry_asm::{Assemble, Raw};
use scry_isa::{AluVariant, Instruction, Instruction::*};
use std::io::Cursor;

macro_rules! test_raw {
    (
		$name:ident [ $($asm:tt)* ] $instructions:expr
	) => {
        #[test]
        fn $name() {
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
		Alu(AluVariant::Add, 4.try_into().unwrap()),
		Alu(AluVariant::Sub, 21.try_into().unwrap()),
		EchoLong(100.try_into().unwrap()),
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
		Alu(AluVariant::Inc, 1.try_into().unwrap()),
		Alu(AluVariant::Inc, 0.try_into().unwrap()),
		Duplicate(false, 3.try_into().unwrap(), 16.try_into().unwrap()),
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
		Alu(AluVariant::Inc, 0.try_into().unwrap()),
		Alu(AluVariant::Inc, 1.try_into().unwrap()),
		Alu(AluVariant::Inc, 0.try_into().unwrap()),
		Alu(AluVariant::Inc, 3.try_into().unwrap()),
		Alu(AluVariant::Inc, 2.try_into().unwrap()),
		Alu(AluVariant::Inc, 1.try_into().unwrap()),
		Alu(AluVariant::Inc, 0.try_into().unwrap()),
		Alu(AluVariant::Inc, 28.try_into().unwrap()),
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
		Alu(AluVariant::Inc, 1.try_into().unwrap()),
		Jump(0.try_into().unwrap(),1.try_into().unwrap()),
		Alu(AluVariant::Add, 0.try_into().unwrap()),
		Alu(AluVariant::Sub, 12.try_into().unwrap()),
	]
}
