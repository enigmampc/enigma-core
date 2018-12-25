/// This file is based on https://github.com/paritytech/parity-ethereum/blob/master/ethcore/wasm/src/env.rs
/// This is Enigma glue for wasmi interpreter

extern crate wasmi;
use std::cell::RefCell;
use std::borrow::ToOwned;

use wasmi::{FuncInstance, Signature, FuncRef, Error, ModuleImportResolver, MemoryInstance, memory_units, MemoryRef, MemoryDescriptor};

pub mod ids {
    pub const RET_FUNC: usize = 1;
    pub const WRITE_STATE_FUNC: usize = 2;
    pub const READ_STATE_FUNC: usize = 3;
    pub const FROM_MEM_FUNC: usize = 4;
    pub const EPRINT_FUNC: usize = 5;
    pub const NAME_LENGTH_FUNC: usize = 6;
    pub const NAME_FUNC: usize = 7;
    pub const ARGS_FUNC: usize = 8;
    pub const ARGS_LENGTH_FUNC: usize = 9;
    pub const TYPES_LENGTH_FUNC: usize = 10;
    pub const TYPES_FUNC: usize = 11;
    pub const WRITE_PAYLOAD_FUNC: usize = 12;
    pub const WRITE_ADDRESS_FUNC: usize = 13;
    pub const GAS_FUNC: usize = 14;
}

pub mod signatures {
    use wasmi::{self, ValueType};
    use wasmi::ValueType::*;

    pub struct StaticSignature(pub &'static [ValueType], pub Option<ValueType>);

    pub const RET: StaticSignature = StaticSignature(
        &[I32, I32],
        None,
    );

    pub const WRITE_STATE: StaticSignature = StaticSignature(
        &[I32, I32, I32, I32],
        None,
    );

    pub const READ_STATE: StaticSignature = StaticSignature(
        &[I32, I32],
        Some(I32),
    );

    pub const FROM_MEM: StaticSignature = StaticSignature(
        &[I32, I32],
        None,
    );

    pub const EPRINT: StaticSignature = StaticSignature(
        &[I32, I32],
        None,
    );

    pub const NAME_LENGTH: StaticSignature = StaticSignature(
        &[],
        Some(I32),
    );

    pub const NAME: StaticSignature = StaticSignature(
        &[I32],
        None,
    );

    pub const ARGS_LENGTH: StaticSignature = StaticSignature(
        &[],
        Some(I32),
    );

    pub const ARGS: StaticSignature = StaticSignature(
        &[I32],
        None,
    );

    pub const TYPES_LENGTH: StaticSignature = StaticSignature(
        &[],
        Some(I32),
    );

    pub const WRITE_PAYLOAD: StaticSignature = StaticSignature(
        &[I32, I32],
        None,
    );

    pub const WRITE_ADDRESS: StaticSignature = StaticSignature(
        &[I32],
        None,
    );

    pub const TYPES: StaticSignature = StaticSignature(
        &[I32],
        None,
    );

    pub const GAS: StaticSignature = StaticSignature(
        &[I32],
        None,
    );

    impl Into<wasmi::Signature> for StaticSignature {
        fn into(self) -> wasmi::Signature {
            wasmi::Signature::new(self.0, self.1)
        }
    }
}

/// Import resolver for wasmi
/// Maps all functions that runtime support to the corresponding contract import
/// entries.
/// Also manages initial memory request from the runtime.
#[derive(Default, Debug)]
pub struct ImportResolver {
    max_memory: u32,
    memory: RefCell<Option<MemoryRef>>,
}

impl ImportResolver {
    /// New import resolver with specifed maximum amount of inital memory (in wasm pages = 64kb)
    pub fn with_limit(max_memory: u32) -> ImportResolver {
        ImportResolver {
            max_memory: max_memory,
            memory: RefCell::new(None),
        }
    }

    /// Returns memory that was instantiated during the contract module
    /// start. If contract does not use memory at all, the dummy memory of length (0, 0)
    /// will be created instead. So this method always returns memory instance
    /// unless errored.
    pub fn memory_ref(&self) -> MemoryRef {
        {
            let mut mem_ref = self.memory.borrow_mut();
            if mem_ref.is_none() {
                *mem_ref = Some(
                    MemoryInstance::alloc(
                        memory_units::Pages(0),
                        Some(memory_units::Pages(0)),
                    ).expect("Memory allocation (0, 0) should not fail; qed")
                );
            }
        }

        self.memory.borrow().clone().expect("it is either existed or was created as (0, 0) above; qed")
    }

    /// Returns memory size module initially requested
    pub fn memory_size(&self) -> Result<u32, Error> {
        Ok(self.memory_ref().current_size().0 as u32)
    }
}


impl ModuleImportResolver for ImportResolver {
    fn resolve_func(&self, field_name: &str, _signature: &Signature) -> Result<FuncRef, Error> {
        let func_ref = match field_name {
            "ret" => FuncInstance::alloc_host(signatures::RET.into(), ids::RET_FUNC),
            "write_state" => FuncInstance::alloc_host(signatures::WRITE_STATE.into(), ids::WRITE_STATE_FUNC),
            "read_state" => FuncInstance::alloc_host(signatures::READ_STATE.into(), ids::READ_STATE_FUNC),
            "from_memory" => FuncInstance::alloc_host(signatures::FROM_MEM.into(), ids::FROM_MEM_FUNC),
            "eprint" => FuncInstance::alloc_host(signatures::EPRINT.into(), ids::EPRINT_FUNC),
            "fetch_function_name_length" => FuncInstance::alloc_host(signatures::NAME_LENGTH.into(), ids::NAME_LENGTH_FUNC),
            "fetch_function_name" => FuncInstance::alloc_host(signatures::NAME.into(), ids::NAME_FUNC),
            "fetch_args_length" => FuncInstance::alloc_host(signatures::ARGS_LENGTH.into(), ids::ARGS_LENGTH_FUNC),
            "fetch_args" => FuncInstance::alloc_host(signatures::ARGS.into(), ids::ARGS_FUNC),
            "fetch_types_length" => FuncInstance::alloc_host(signatures::TYPES_LENGTH.into(), ids::TYPES_LENGTH_FUNC),
            "fetch_types" => FuncInstance::alloc_host(signatures::TYPES.into(), ids::TYPES_FUNC),
            "write_payload" => FuncInstance::alloc_host(signatures::WRITE_PAYLOAD.into(), ids::WRITE_PAYLOAD_FUNC),
            "write_address" => FuncInstance::alloc_host(signatures::WRITE_ADDRESS.into(), ids::WRITE_ADDRESS_FUNC),
            "gas" => FuncInstance::alloc_host(signatures::GAS.into(), ids::GAS_FUNC),
            _ => {
                return Err(wasmi::Error::Instantiation(
                    format!("Export {} not found", field_name),
                ))
            }
        };

        Ok(func_ref)
    }

    fn resolve_memory(
        &self,
        field_name: &str,
        descriptor: &MemoryDescriptor,
    ) -> Result<MemoryRef, Error> {
        if field_name == "memory" {
            let effective_max = descriptor.maximum().unwrap_or(self.max_memory + 1);
            if descriptor.initial() > self.max_memory || effective_max > self.max_memory
                {
                    Err(Error::Instantiation("Module requested too much memory".to_owned()))
                } else {
                let mem = MemoryInstance::alloc(
                    memory_units::Pages(descriptor.initial() as usize),
                    descriptor.maximum().map(|x| memory_units::Pages(x as usize)),
                )?;
                *self.memory.borrow_mut() = Some(mem.clone());
                Ok(mem)
            }
        } else {
            Err(Error::Instantiation("Memory imported under unknown name".to_owned()))
        }
    }
}
