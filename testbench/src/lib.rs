use std::sync::OnceLock;

use riscv_core::VerilatorRuntime;

fn cached_runtime(
    cache: &'static OnceLock<VerilatorRuntime>,
    create_runtime: fn() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>,
    name: &str,
) -> &'static VerilatorRuntime {
    cache.get_or_init(|| {
        create_runtime().unwrap_or_else(|err| panic!("Failed to create {name} runtime: {err}"))
    })
}

macro_rules! cached_runtime_fn {
    ($fn_name:ident, $create_fn:ident, $name:literal) => {
        pub fn $fn_name() -> &'static VerilatorRuntime {
            static RUNTIME: OnceLock<VerilatorRuntime> = OnceLock::new();
            cached_runtime(&RUNTIME, riscv_core::$create_fn, $name)
        }
    };
}

cached_runtime_fn!(alu_runtime, create_alu_runtime, "ALU");
cached_runtime_fn!(fpu_runtime, create_fpu_runtime, "FPU");
cached_runtime_fn!(fpu_classifier_runtime, create_fpu_classifier_runtime, "FPU classifier");
cached_runtime_fn!(fpu_comparator_runtime, create_fpu_comparator_runtime, "FPU comparator");
cached_runtime_fn!(
    fpu_float_to_int_runtime,
    create_fpu_float_to_int_runtime,
    "FPU float-to-int"
);
cached_runtime_fn!(
    fpu_int_to_float_runtime,
    create_fpu_int_to_float_runtime,
    "FPU int-to-float"
);
cached_runtime_fn!(fpu_sqrt_runtime, create_fpu_sqrt_runtime, "FPU sqrt");
cached_runtime_fn!(system_controller_runtime, create_system_controller_runtime, "system controller");
cached_runtime_fn!(uart_runtime, create_uart_runtime, "UART");
cached_runtime_fn!(uart_1m_runtime, create_uart_1m_runtime, "1M UART");
