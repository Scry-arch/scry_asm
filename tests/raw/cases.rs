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
            assert_eq!(assembled, expected_bytes, "Assembled code on the left.");
        }
    };
}

macro_rules! test_raw_fail {
    (
		$name:ident { $($asm:literal)* } $err_msg:literal
	) => {
        #[test]
        fn $name() {
            // We first assemble the string
            let assembled = Raw::assemble([
				$($asm),*
			].into_iter());

			// Check that that an error message is returned, with checking the error
			assert_eq!(assembled, Err($err_msg.to_string()));
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
				"cap =>0, =>0"
				"cap =>0, =>0"
	 "jmpAt:"
				"cap =>0, =>0"
				"cap =>0, =>0"
				"cap =>0, =>0"
				"cap =>0, =>0"
	 "jmpTo:"	"sub =>0"
	}
	[
		Alu(AluVariant::Inc, 3.try_into().unwrap());
		Jump(4.try_into().unwrap(),2.try_into().unwrap());
		Instruction::nop();
		Instruction::nop();
		Instruction::nop();
		Instruction::nop();
		Instruction::nop();
		Instruction::nop();
		Alu(AluVariant::Sub, 0.try_into().unwrap());
	]
}

test_raw! {
	jmp_to_before_jmp
	{
					"inc =>jmpAt=>loop=>inc_to"
		"loop:"		"jmp loop, jmpAt"
					"cap =>0, =>0"
		"inc_to:"	"dec =>3"
					"cap =>0, =>0"
					"sub =>0"
		"jmpAt:"
	}
	[
		Alu(AluVariant::Inc, 7.try_into().unwrap());
		Jump(0.try_into().unwrap(),4.try_into().unwrap());
		Instruction::nop();
		Alu(AluVariant::Dec, 3.try_into().unwrap());
		Instruction::nop();
		Alu(AluVariant::Sub, 0.try_into().unwrap());
	]
}

test_raw! {
	jmp_to_jmp
	{
					"inc =>jmpAt=>loop=>inc_to"
		"loop:"		"cap =>0, =>0"
					"jmp loop, jmpAt"
					"cap =>0, =>0"
		"inc_to:"	"dec =>jmpAt=>loop"
					   "sub =>0"
		"jmpAt:"
	}
	[
		Alu(AluVariant::Inc, 8.try_into().unwrap());
		Instruction::nop();
		Jump((-1).try_into().unwrap(),3.try_into().unwrap());
		Instruction::nop();
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
	bytes_assembler_directive
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

test_raw! {
	bytes_assembler_directive_references
	{
	"lab0:"		".bytes u0, lab2"
				".bytes i0, lab4=>lab2"
	"lab2:"		"add =>4"
	"lab4:"		".bytes u1, lab18"
				".bytes u2, lab4=>lab12"
				"sub =>21"
	"lab12:"	".bytes i1, lab12=>lab18"
				".bytes i2, lab18=>lab0"
	"lab18:"	"echo =>100"
	}
	[
		2u8;
		-2i8;
		Alu(AluVariant::Add, 4.try_into().unwrap());
		18u16;
		8u32;
		Alu(AluVariant::Sub, 21.try_into().unwrap());
		6i16;
		-18i32;
		EchoLong(100.try_into().unwrap());
	]
}

test_raw! {
	bytes_followed_by_label_reference
	{
					".bytes u1, 0"
					"inc =>dup_addr"
	"dup_addr:"		"cap =>0, =>0"
	}
	[
		0u16;
		Alu(AluVariant::Inc, 0.try_into().unwrap());
		Instruction::nop();
	]
}

test_raw_fail! {
	ret_trigger_before_instr
	{
		"before_ret:"
						"add =>0"
						"ret before_ret"
	}
	"Invalid Value (Should be 0 - 63): -1\nSource: before_ret"
}
test_raw_fail! {
	const_invalid_label
	{
		"const u0, cmp_fn_addr"
	}
	"Unknown label: cmp_fn_addr"
}
