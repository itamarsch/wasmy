use std::{
    cell::{Cell, RefCell},
    ops::DerefMut,
    process::exit,
};

use crate::{
    module::{functions::Function, Module},
    runtime::{locals::Locals, value::Value},
    types::{BlockIdx, BlockType, FuncIdx, Instruction, NumericValueType, ValueType},
};

use self::{function_state::FunctionState, stack::Stack};

pub mod function_state;
mod local;
mod locals;
mod stack;
mod value;

pub struct Runtime<'a> {
    stack: RefCell<Stack>,
    module: Module<'a>,
    current_function_state: RefCell<FunctionState>,
    function_depth: Cell<usize>,
}

impl<'a> Runtime<'a> {
    pub fn new(module: Module<'a>) -> Self {
        let (start_idx, Function::Local(starting_function)) = module.get_starting_function() else {
            panic!("Cannot start from imported function")
        };
        assert!(
            starting_function.signature.params.is_empty(),
            "_start function cannot take arguments"
        );

        let stack = RefCell::new(Stack::new());
        let initial_function_state = RefCell::new(FunctionState::new_function(
            Locals::new_no_function_parameters(&starting_function.code.locals),
            start_idx,
        ));

        Runtime {
            stack,
            module,
            current_function_state: initial_function_state,
            function_depth: Cell::new(0),
        }
    }

    fn wasi_function(&self, name: &str) {
        match name {
            "proc_exit" => {
                let exit_code = self.stack.borrow_mut().pop_i32();
                exit(exit_code);
            }
            _ => {
                panic!("Unknown wasi function: {}", name);
            }
        }
    }

    fn call_function(&self, func_idx: FuncIdx) {
        let next_function = self
            .module
            .get_function(func_idx)
            .expect("Call instruction to have a valid function index");
        match next_function {
            Function::Local(function) => {
                self.function_depth.set(self.function_depth.get() + 1);
                let locals = Locals::new(
                    &function.code.locals,
                    &function.signature.params,
                    self.stack.borrow_mut().deref_mut(),
                );

                let mut new_function_state = FunctionState::new_function(locals, func_idx);

                std::mem::swap(
                    &mut new_function_state,
                    self.current_function_state.borrow_mut().deref_mut(),
                );
                self.stack
                    .borrow_mut()
                    .push_function_state(new_function_state);
            }
            Function::Imported(function) => {
                if function.mod_name == "wasi_snapshot_preview1" {
                    self.wasi_function(function.name);
                }
            }
        }
    }

    fn run_instruction(&self, instruction: &Instruction) {
        match instruction {
            Instruction::Block(block_type, block_idx) => {
                self.execute_block(*block_idx, *block_type)
            }
            Instruction::If {
                block_type,
                if_expr,
                else_expr,
            } => {
                let condition = self.stack.borrow_mut().pop_bool();
                if condition {
                    self.execute_block(*if_expr, *block_type);
                } else {
                    self.execute_block(*else_expr, *block_type);
                }
            }
            Instruction::Call(func_idx) => {
                self.call_function(*func_idx);
            }
            Instruction::Drop => {
                self.stack.borrow_mut().drop();
            }
            Instruction::LocalGet(idx) => {
                let value = self.current_function_state.borrow().get_local_value(*idx);
                self.stack.borrow_mut().push_value(value);
            }
            Instruction::LocalSet(idx) => {
                let value = self.stack.borrow_mut().pop_value();
                self.current_function_state
                    .borrow_mut()
                    .set_local_value(*idx, value);
            }

            Instruction::I32Const(value) => {
                self.stack.borrow_mut().push_i32(*value);
            }
            Instruction::I64Const(value) => {
                self.stack.borrow_mut().push_i64(*value);
            }
            Instruction::F32Const(value) => {
                self.stack.borrow_mut().push_f32(*value);
            }
            Instruction::F64Const(value) => {
                self.stack.borrow_mut().push_f64(*value);
            }
            Instruction::I32Add => {
                let b = self.stack.borrow_mut().pop_i32();
                let a = self.stack.borrow_mut().pop_i32();

                self.stack.borrow_mut().push_i32(a.wrapping_add(b));
            }
            Instruction::I32Sub => {
                let b = self.stack.borrow_mut().pop_i32();
                let a = self.stack.borrow_mut().pop_i32();

                self.stack.borrow_mut().push_i32(a.wrapping_sub(b));
            }
            Instruction::I32Mul => {
                let b = self.stack.borrow_mut().pop_i32();
                let a = self.stack.borrow_mut().pop_i32();

                self.stack.borrow_mut().push_i32(a.wrapping_mul(b));
            }
            Instruction::I32DivS => {
                let b = self.stack.borrow_mut().pop_i32();
                let a = self.stack.borrow_mut().pop_i32();

                self.stack.borrow_mut().push_i32(a / b);
            }
            Instruction::I32RemS => {
                let b = self.stack.borrow_mut().pop_i32();
                let a = self.stack.borrow_mut().pop_i32();
                self.stack.borrow_mut().push_i32(a % b);
            }
            Instruction::I64Add => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();
                self.stack.borrow_mut().push_i64(a.wrapping_add(b));
            }
            Instruction::I64Sub => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();

                self.stack.borrow_mut().push_i64(a.wrapping_sub(b));
            }

            Instruction::I64Mul => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();

                self.stack.borrow_mut().push_i64(a.wrapping_mul(b));
            }
            Instruction::I64DivS => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();

                self.stack.borrow_mut().push_i64(a / b);
            }
            Instruction::I64RemS => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();
                self.stack.borrow_mut().push_i64(a % b);
            }
            Instruction::I64ShrS => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();
                self.stack.borrow_mut().push_i64(a >> (b % 64));
            }
            Instruction::I64Shl => {
                let b = self.stack.borrow_mut().pop_i64();
                let a = self.stack.borrow_mut().pop_i64();
                self.stack.borrow_mut().push_i64(a << (b % 64));
            }
            Instruction::F32Add => {
                let b = self.stack.borrow_mut().pop_f32();
                let a = self.stack.borrow_mut().pop_f32();

                self.stack.borrow_mut().push_f32(a + b);
            }

            Instruction::F32Sub => {
                let b = self.stack.borrow_mut().pop_f32();
                let a = self.stack.borrow_mut().pop_f32();

                self.stack.borrow_mut().push_f32(a - b);
            }
            Instruction::F32Mul => {
                let b = self.stack.borrow_mut().pop_f32();
                let a = self.stack.borrow_mut().pop_f32();

                self.stack.borrow_mut().push_f32(a * b);
            }
            Instruction::F32Div => {
                let b = self.stack.borrow_mut().pop_f32();
                let a = self.stack.borrow_mut().pop_f32();

                self.stack.borrow_mut().push_f32(a / b);
            }
            Instruction::F32Sqrt => {
                let a = self.stack.borrow_mut().pop_f32();
                self.stack.borrow_mut().push_f32(a.sqrt());
            }
            Instruction::F64Add => {
                let b = self.stack.borrow_mut().pop_f64();
                let a = self.stack.borrow_mut().pop_f64();

                self.stack.borrow_mut().push_f64(a + b);
            }
            Instruction::F64Sub => {
                let b = self.stack.borrow_mut().pop_f64();
                let a = self.stack.borrow_mut().pop_f64();

                self.stack.borrow_mut().push_f64(a - b);
            }
            Instruction::F64Mul => {
                let b = self.stack.borrow_mut().pop_f64();
                let a = self.stack.borrow_mut().pop_f64();

                self.stack.borrow_mut().push_f64(a * b);
            }
            Instruction::F64Div => {
                let b = self.stack.borrow_mut().pop_f64();
                let a = self.stack.borrow_mut().pop_f64();

                self.stack.borrow_mut().push_f64(a / b);
            }

            Instruction::I32WrapI64 => {
                let a = self.stack.borrow_mut().pop_i64();
                self.stack.borrow_mut().push_i32(a as i32);
            }
            Instruction::I32TruncF32S => {
                let a = self.stack.borrow_mut().pop_f32();
                self.stack.borrow_mut().push_i32(a as i32);
            }
            Instruction::I32TruncF64S => {
                let a = self.stack.borrow_mut().pop_f64();
                self.stack.borrow_mut().push_i32(a as i32);
            }
            Instruction::I64ExtendI32S => {
                let a = self.stack.borrow_mut().pop_i32();
                self.stack.borrow_mut().push_i64(a as i64);
            }
            Instruction::I64TruncF32S => {
                let a = self.stack.borrow_mut().pop_f32();
                self.stack.borrow_mut().push_i64(a as i64);
            }
            Instruction::F32ConvertI32S => {
                let a = self.stack.borrow_mut().pop_i32();
                self.stack.borrow_mut().push_f32(a as f32);
            }
            _ => panic!(
                "Instruction: {:?} not implemented {:?}",
                instruction, self.stack
            ),
        }
        println!(
            "Executed: {:?}, current state: {:?}, stack: {:?}",
            instruction,
            "",
            "" // self.current_function_state.borrow(),
               // self.stack.borrow()
        );
    }

    fn execute_block(&self, block_idx: BlockIdx, block_type: BlockType) {
        let mut new_function_state = self
            .current_function_state
            .borrow()
            .new_block(block_idx, block_type);
        std::mem::swap(
            &mut new_function_state,
            self.current_function_state.borrow_mut().deref_mut(),
        );
        self.stack
            .borrow_mut()
            .push_function_state(new_function_state);
    }
    fn pop_returns(&self, signature_returns: &[ValueType]) -> Vec<Value> {
        let amount_of_returns = signature_returns.len();
        let mut returns = Vec::with_capacity(amount_of_returns);
        for _ in 0..amount_of_returns {
            let value = self.stack.borrow_mut().pop_value();
            returns.push(value);
        }

        for (signature, value) in signature_returns.iter().zip(returns.iter().rev()) {
            match (signature, value) {
                (ValueType::Numeric(NumericValueType::I32), Value::I32(_))
                | (ValueType::Numeric(NumericValueType::I64), Value::I64(_))
                | (ValueType::Numeric(NumericValueType::F32), Value::F32(_))
                | (ValueType::Numeric(NumericValueType::F64), Value::F64(_)) => {}
                _ => {
                    panic!("Returns don't match signature of function, expected value of type: {:?} received: {:?}", signature, value);
                }
            }
        }
        returns
    }

    fn reassemble_returns(&self, returns: &mut Vec<Value>) {
        for _ in 0..returns.len() {
            self.stack
                .borrow_mut()
                .push_value(returns.pop().expect("Pushed enough elements"));
        }
    }

    fn return_from_function(&self, signature_returns: &[ValueType]) {
        let mut returns = self.pop_returns(signature_returns);
        let function_state = self.stack.borrow_mut().pop_function_state();
        self.reassemble_returns(&mut returns);
        *self.current_function_state.borrow_mut() = function_state;
    }

    pub fn execute(self) {
        loop {
            let Some(Function::Local(current_function)) = self
                .module
                .get_function(self.current_function_state.borrow().function_idx())
            else {
                unreachable!(
                    "Current runing function cannot be imported and its index has to exist"
                )
            };

            if current_function
                .code
                .instructions
                .done(self.current_function_state.borrow().instruction_index())
            {
                if self.function_depth.get() > 0 || self.current_function_state.borrow().in_block()
                {
                    let borrow = self.current_function_state.borrow();
                    let index = borrow.instruction_index();
                    drop(borrow);

                    match index {
                        function_state::InstructionIndex::IndexInFunction(_) => {
                            self.return_from_function(&current_function.signature.returns);
                            self.function_depth.set(self.function_depth.get() - 1);
                        }
                        function_state::InstructionIndex::IndexInBlock(_, block_type, _) => {
                            let block_type_slice = match block_type.0 {
                                Some(t) => &[t][..],
                                None => &[][..],
                            };
                            self.return_from_function(block_type_slice)
                        }
                    }
                    continue;
                } else {
                    break;
                }
            }

            let instruction = &current_function
                .code
                .instructions
                .get_instruction(self.current_function_state.borrow().instruction_index());

            self.current_function_state.borrow_mut().next_instruction();

            self.run_instruction(instruction);
        }
    }
}
