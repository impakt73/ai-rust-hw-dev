use std::cell::OnceCell;
use std::thread::LocalKey;

use riscv_core::{AsVerilatedModel, VerilatedModelConfig, VerilatorRuntime};

const TESTBENCH_VERILATOR_OPT_LEVEL_ENV: &str = "TESTBENCH_VERILATOR_OPT_LEVEL";
const DEFAULT_TESTBENCH_VERILATOR_OPT_LEVEL: usize = 0;

fn testbench_model_config() -> VerilatedModelConfig {
    let verilator_optimization = std::env::var(TESTBENCH_VERILATOR_OPT_LEVEL_ENV)
        .ok()
        .map(|value| {
            value.parse::<usize>().unwrap_or_else(|err| {
                panic!("Invalid {TESTBENCH_VERILATOR_OPT_LEVEL_ENV} value `{value}`: {err}")
            })
        })
        .unwrap_or(DEFAULT_TESTBENCH_VERILATOR_OPT_LEVEL);

    assert!(
        verilator_optimization <= 3,
        "{TESTBENCH_VERILATOR_OPT_LEVEL_ENV} must be between 0 and 3, got {verilator_optimization}"
    );

    VerilatedModelConfig {
        verilator_optimization,
        ..VerilatedModelConfig::default()
    }
}

pub fn create_testbench_model<'ctx, M: AsVerilatedModel<'ctx>>(
    runtime: &'ctx VerilatorRuntime,
) -> Result<M, Box<dyn std::error::Error>> {
    runtime
        .create_model(&testbench_model_config())
        .map_err(|err| err.into())
}

/// Executes `f` with a per-thread cached Verilator runtime for the requested DUT family.
///
/// The cache avoids repeated runtime construction within long-running integration binaries while
/// keeping each runtime confined to a single test worker thread.
fn with_cached_runtime<T>(
    cache: &'static LocalKey<OnceCell<VerilatorRuntime>>,
    create_runtime: fn() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>,
    name: &str,
    f: impl FnOnce(&VerilatorRuntime) -> T,
) -> T {
    cache.with(|runtime| {
        let runtime = runtime.get_or_init(|| {
            create_runtime().unwrap_or_else(|err| panic!("Failed to create {name} runtime: {err}"))
        });

        f(runtime)
    })
}

macro_rules! with_cached_model_fn {
    ($fn_name:ident, $model_ty:ident, $create_fn:ident, $name:literal) => {
        pub fn $fn_name<T>(f: impl for<'ctx> FnOnce(riscv_core::$model_ty<'ctx>) -> T) -> T {
            thread_local! {
                static RUNTIME: OnceCell<VerilatorRuntime> = const { OnceCell::new() };
            }

            with_cached_runtime(&RUNTIME, riscv_core::$create_fn, $name, |runtime| {
                let model = create_testbench_model::<riscv_core::$model_ty>(runtime)
                    .unwrap_or_else(|err| panic!("Failed to create {} model: {}", $name, err));
                f(model)
            })
        }
    };
}

with_cached_model_fn!(with_alu_model, Alu, create_alu_runtime, "ALU");
with_cached_model_fn!(with_fpu_model, Fpu, create_fpu_runtime, "FPU");
with_cached_model_fn!(
    with_fpu_classifier_model,
    FpuClassifier,
    create_fpu_classifier_runtime,
    "FPU classifier"
);
with_cached_model_fn!(
    with_fpu_comparator_model,
    FpuComparator,
    create_fpu_comparator_runtime,
    "FPU comparator"
);
with_cached_model_fn!(
    with_fpu_float_to_int_model,
    FpuFloatToInt,
    create_fpu_float_to_int_runtime,
    "FPU float-to-int"
);
with_cached_model_fn!(
    with_fpu_int_to_float_model,
    FpuIntToFloat,
    create_fpu_int_to_float_runtime,
    "FPU int-to-float"
);
with_cached_model_fn!(
    with_fpu_sqrt_model,
    FpuSqrt,
    create_fpu_sqrt_runtime,
    "FPU sqrt"
);
with_cached_model_fn!(
    with_system_controller_model,
    SystemController,
    create_system_controller_runtime,
    "system controller"
);
