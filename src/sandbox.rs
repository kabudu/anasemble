use serde::Serialize;
use wasm_encoder::{
    BlockType, CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
    Module as EncodedModule, TypeSection, ValType,
};
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimitsBuilder};

use crate::canonical::bytes_digest;
use crate::model::{Candidate, Error};

#[derive(Debug, Serialize)]
pub struct SandboxEvidence {
    pub engine: &'static str,
    pub module_digest: String,
    pub imports: usize,
    pub fuel_per_call: u64,
    pub memory_limit_bytes: usize,
    pub verified_executions: usize,
}

pub fn compile(candidate: &Candidate) -> Result<Vec<u8>, Error> {
    let mut module = EncodedModule::new();
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I64]);
    module.section(&types);
    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);
    let mut exports = ExportSection::new();
    exports.export("step", ExportKind::Func, 0);
    module.section(&exports);
    let mut function = Function::new([]);
    for transition in &candidate.transitions {
        let state = index_of(&candidate.grammar.states, &transition.state)?;
        let input = index_of(&candidate.grammar.inputs, &transition.input)?;
        function.instruction(&Instruction::LocalGet(0));
        function.instruction(&Instruction::I32Const(state));
        function.instruction(&Instruction::I32Eq);
        function.instruction(&Instruction::LocalGet(1));
        function.instruction(&Instruction::I32Const(input));
        function.instruction(&Instruction::I32Eq);
        function.instruction(&Instruction::I32And);
        function.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
        let next_state = index_of(&candidate.grammar.states, &transition.next_state)? as i64;
        let output = index_of(&candidate.grammar.outputs, &transition.output)? as i64;
        function.instruction(&Instruction::I64Const((next_state << 32) | output));
        function.instruction(&Instruction::Else);
    }
    function.instruction(&Instruction::I64Const(-1));
    for _ in &candidate.transitions {
        function.instruction(&Instruction::End);
    }
    function.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&function);
    module.section(&code);
    Ok(module.finish())
}

pub fn verify(candidate: &Candidate, wasm: &[u8]) -> Result<SandboxEvidence, Error> {
    const FUEL: u64 = 10_000;
    const MEMORY: usize = 65_536;
    let (engine, module) = load_module(wasm)?;
    let imports = module.imports().count();
    if imports != 0 {
        return Err(Error::CheckerRejected(
            "candidate WebAssembly requests ambient capabilities".into(),
        ));
    }
    let limits = StoreLimitsBuilder::new()
        .memory_size(MEMORY)
        .instances(1)
        .memories(1)
        .tables(0)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|limits| limits);
    store
        .set_fuel(FUEL)
        .map_err(|error| Error::CheckerRejected(error.to_string()))?;
    let instance = Linker::new(&engine)
        .instantiate_and_start(&mut store, &module)
        .map_err(|error| {
            Error::CheckerRejected(format!("sandbox instantiation failed: {error}"))
        })?;
    let step = instance
        .get_typed_func::<(i32, i32), i64>(&store, "step")
        .map_err(|error| Error::CheckerRejected(format!("sandbox ABI mismatch: {error}")))?;
    for transition in &candidate.transitions {
        store
            .set_fuel(FUEL)
            .map_err(|error| Error::CheckerRejected(error.to_string()))?;
        let state = index_of(&candidate.grammar.states, &transition.state)?;
        let input = index_of(&candidate.grammar.inputs, &transition.input)?;
        let observed = step.call(&mut store, (state, input)).map_err(|error| {
            Error::CheckerRejected(format!("sandbox execution failed: {error}"))
        })?;
        let expected = ((index_of(&candidate.grammar.states, &transition.next_state)? as i64)
            << 32)
            | index_of(&candidate.grammar.outputs, &transition.output)? as i64;
        if observed != expected {
            return Err(Error::CheckerRejected(
                "sandbox behavior disagrees with the certified candidate".into(),
            ));
        }
    }
    Ok(SandboxEvidence {
        engine: "wasmi-1.1.0",
        module_digest: bytes_digest(wasm),
        imports,
        fuel_per_call: FUEL,
        memory_limit_bytes: MEMORY,
        verified_executions: candidate.transitions.len(),
    })
}

pub fn rejects_capabilities(wasm: &[u8]) -> Result<(), Error> {
    let (_, module) = load_module(wasm)?;
    if module.imports().next().is_some() {
        return Err(Error::CheckerRejected(
            "candidate WebAssembly requests ambient capabilities".into(),
        ));
    }
    Ok(())
}

pub fn execute_untrusted_step(
    wasm: &[u8],
    state: i32,
    input: i32,
    fuel: u64,
) -> Result<i64, Error> {
    let (engine, module) = load_module(wasm)?;
    if module.imports().next().is_some() {
        return Err(Error::CheckerRejected(
            "candidate WebAssembly requests ambient capabilities".into(),
        ));
    }
    let limits = StoreLimitsBuilder::new()
        .memory_size(65_536)
        .instances(1)
        .memories(1)
        .tables(0)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|limits| limits);
    store
        .set_fuel(fuel)
        .map_err(|error| Error::CheckerRejected(error.to_string()))?;
    let instance = Linker::new(&engine)
        .instantiate_and_start(&mut store, &module)
        .map_err(|error| {
            Error::CheckerRejected(format!("sandbox instantiation failed: {error}"))
        })?;
    let step = instance
        .get_typed_func::<(i32, i32), i64>(&store, "step")
        .map_err(|error| Error::CheckerRejected(format!("sandbox ABI mismatch: {error}")))?;
    step.call(&mut store, (state, input)).map_err(|error| {
        Error::SearchExhausted(format!("sandbox fuel or execution bound reached: {error}"))
    })
}

fn load_module(wasm: &[u8]) -> Result<(Engine, Module), Error> {
    if wasm.len() > 1_048_576 {
        return Err(Error::SearchExhausted(
            "WebAssembly module exceeds the 1 MiB sandbox bound".into(),
        ));
    }
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm)
        .map_err(|error| Error::CheckerRejected(format!("invalid WebAssembly: {error}")))?;
    Ok((engine, module))
}

fn index_of(values: &[String], value: &str) -> Result<i32, Error> {
    values
        .iter()
        .position(|item| item == value)
        .and_then(|index| i32::try_from(index).ok())
        .ok_or_else(|| Error::InvalidEvidence("candidate uses an unknown DSL symbol".into()))
}
