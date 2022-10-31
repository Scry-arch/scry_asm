use duplicate::duplicate_item;
use scry_asm::{Assemble, Raw};
use scry_isa::{AluVariant, Bits, CallVariant, Instruction, Instruction::*};

trait ByteBlock
{
	fn into_bytes(self) -> Vec<u8>;
}

impl ByteBlock for Instruction
{
	fn into_bytes(self) -> Vec<u8>
	{
		self.encode().to_le_bytes().into_iter().collect()
	}
}

#[duplicate_item(
	typ; [u8]; [u16]; [u32]; [u64]; [i8]; [i16]; [i32]; [i64]
)]
impl ByteBlock for typ
{
	fn into_bytes(self) -> Vec<u8>
	{
		self.to_le_bytes().into_iter().collect()
	}
}

macro_rules! test_raw {
    (
		$name:ident { $($asm:literal)* } [$($instructions:expr;)+]
	) => {
        #[test]
        fn $name() {
            // We first assemble the string
            let assembled = Raw::assemble([
				$($asm),*
			].into_iter()).unwrap();

			// Build expected bytes
			let mut expected_bytes = Vec::new();

			$(
				expected_bytes.extend($instructions.into_bytes().into_iter());
			)+

            // Ensure they were encoding is equivalent to the expected instructions
            assert_eq!(assembled, expected_bytes);
        }
    };
}

test_raw! {
	independent_instructions
	{
		"add =>4"
		"sub =>21"
		"echo =>100"
	}
	[
		Alu(AluVariant::Add, 4.try_into().unwrap());
		Alu(AluVariant::Sub, 21.try_into().unwrap());
		EchoLong(100.try_into().unwrap());
	]
}

test_raw! {
	output_to_next
	{
				"inc =>instr2"
				"inc =>instr2"
	 "instr2:"	"dup =>3, =>16"
	}
	[
		Alu(AluVariant::Inc, 1.try_into().unwrap());
		Alu(AluVariant::Inc, 0.try_into().unwrap());
		Duplicate(false, 3.try_into().unwrap(), 16.try_into().unwrap());
	]
}

test_raw! {
	multiple_label_targets
	{
				"inc =>instr"
	 "instr:"	"inc =>instr2"
				"inc =>instr2"
	 "instr2:"	"inc =>instr3"
				"inc =>instr3"
				"inc =>instr3"
				"inc =>instr3"
	 "instr3:"	"inc =>28"
	}
	[
		Alu(AluVariant::Inc, 0.try_into().unwrap());
		Alu(AluVariant::Inc, 1.try_into().unwrap());
		Alu(AluVariant::Inc, 0.try_into().unwrap());
		Alu(AluVariant::Inc, 3.try_into().unwrap());
		Alu(AluVariant::Inc, 2.try_into().unwrap());
		Alu(AluVariant::Inc, 1.try_into().unwrap());
		Alu(AluVariant::Inc, 0.try_into().unwrap());
		Alu(AluVariant::Inc, 28.try_into().unwrap());
	]
}

test_raw! {
	skip_one_using_jmp
	{
				"inc =>jmpAt=>jmpTo"
				"jmp jmpTo, jmpAt"
	 "jmpAt:"
				"add =>0"
	 "jmpTo:"	"sub =>12"
	}
	[
		Alu(AluVariant::Inc, 1.try_into().unwrap());
		Jump(1.try_into().unwrap(),0.try_into().unwrap());
		Alu(AluVariant::Add, 0.try_into().unwrap());
		Alu(AluVariant::Sub, 12.try_into().unwrap());
	]
}

test_raw! {
	skip_multiple_using_jmp
	{
				"inc =>jmpAt=>jmpTo"
				"jmp jmpTo, jmpAt"
				"nop"
				"nop"
	 "jmpAt:"
				"nop"
				"nop"
				"nop"
				"nop"
	 "jmpTo:"	"sub =>0"
	}
	[
		Alu(AluVariant::Inc, 3.try_into().unwrap());
		Jump(4.try_into().unwrap(),2.try_into().unwrap());
		Nop;
		Nop;
		Nop;
		Nop;
		Nop;
		Nop;
		Alu(AluVariant::Sub, 0.try_into().unwrap());
	]
}

test_raw! {
	jmp_to_before_jmp
	{
					"inc =>jmpAt=>loop=>inc_to"
		"loop:"		"jmp loop, jmpAt"
					"nop"
		"inc_to:"	"dec =>3"
					"nop"
					"sub =>0"
		"jmpAt:"
	}
	[
		Alu(AluVariant::Inc, 7.try_into().unwrap());
		Jump(0.try_into().unwrap(),4.try_into().unwrap());
		Nop;
		Alu(AluVariant::Dec, 3.try_into().unwrap());
		Nop;
		Alu(AluVariant::Sub, 0.try_into().unwrap());
	]
}

test_raw! {
	jmp_to_jmp
	{
					"inc =>jmpAt=>loop=>inc_to"
		"loop:"		"nop"
					"jmp loop, jmpAt"
					"nop"
		"inc_to:"	"dec =>jmpAt=>loop"
					   "sub =>0"
		"jmpAt:"
	}
	[
		Alu(AluVariant::Inc, 8.try_into().unwrap());
		Nop;
		Jump((-1).try_into().unwrap(),3.try_into().unwrap());
		Nop;
		Alu(AluVariant::Dec, 1.try_into().unwrap());
		Alu(AluVariant::Sub, 0.try_into().unwrap());
	]
}

test_raw! {
	return_and_const_in_middle
	{
						"inc =>to_add"
						"ret return_at"
						"const i0, 2"
		"to_add:"		"add =>0"
		"return_at:"
	}
	[
		Alu(AluVariant::Inc, 2.try_into().unwrap());
		Call(CallVariant::Ret, 2.try_into().unwrap());
		Constant(Bits::<8,true>::try_from(2).unwrap().into());
		Alu(AluVariant::Add, 0.try_into().unwrap());
	]
}

test_raw! {
	const_assembler_directive
	{
		".bytes u0, 0"
		".bytes i0, 1"
		"add =>4"
		".bytes u1, 2456"
		".bytes u2, 123762"
		"sub =>21"
		".bytes i1, -123"
		".bytes i2, -7612"
		"echo =>100"
	}
	[
		0u8;
		1i8;
		Alu(AluVariant::Add, 4.try_into().unwrap());
		2456u16;
		123762u32;
		Alu(AluVariant::Sub, 21.try_into().unwrap());
		-123i16;
		-7612i32;
		EchoLong(100.try_into().unwrap());
	]
}
