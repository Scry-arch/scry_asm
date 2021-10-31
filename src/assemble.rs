pub trait Assemble {
    type Error;

    fn assemble<'a, I>(asm: I) -> Result<Vec<u8>, Self::Error>
    where
        I: Iterator<Item = &'a str> + Clone;
}

pub trait Disassemble {
    type Error;

    fn disassemble<'a, I>(asm: I) -> Result<String, Self::Error>
    where
        I: Iterator<Item = &'a u8> + Clone;
}
