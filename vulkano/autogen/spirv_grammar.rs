// Copyright (c) 2021 The Vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use serde::Deserialize;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

#[derive(Clone, Debug, Deserialize)]
pub struct SpirvGrammar {
    pub major_version: u16,
    pub minor_version: u16,
    pub revision: u16,
    pub instructions: Vec<SpirvInstruction>,
    pub operand_kinds: Vec<SpirvOperandKind>,
}

impl SpirvGrammar {
    pub fn new<P: AsRef<Path> + ?Sized>(path: &P) -> Self {
        let mut reader = BufReader::new(File::open(path).unwrap());
        let mut json = String::new();
        reader.read_to_string(&mut json).unwrap();
        serde_json::from_str(&json).unwrap()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpirvInstruction {
    pub opname: String,
    #[allow(unused)] // to keep my build system from yelling at me
    pub class: String,
    pub opcode: u16,
    #[serde(default)]
    pub operands: Vec<SpirvOperand>,
    #[serde(default)]
    #[allow(unused)] // to keep my build system from yelling at me
    pub capabilities: Vec<String>,
    #[serde(default)]
    #[allow(unused)] // to keep my build system from yelling at me
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpirvOperand {
    pub kind: String,
    pub quantifier: Option<char>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpirvOperandKind {
    pub category: String,
    pub kind: String,
    #[serde(default)]
    pub enumerants: Vec<SpirvKindEnumerant>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpirvKindEnumerant {
    pub enumerant: String,
    pub value: Value,
    #[serde(default)]
    pub parameters: Vec<SpirvParameter>,
    #[serde(default)]
    #[allow(unused)] // to keep my build system from yelling at me
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpirvParameter {
    pub kind: String,
    pub name: Option<String>,
}
