use std::cell::OnceCell;
use std::thread::LocalKey;

use riscv_core::VerilatorRuntime;

fn with_cached_runtime<T>(
    cache: &'static LocalKey<OnceCell<VerilatorRuntime>>,
    create_runtime: fn() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>,
    name: &str,
    f: impl FnOnce(&'static VerilatorRuntime) -> T,
) -> T {
    cache.with(|runtime| {
        let runtime = runtime.get_or_init(|| {
            create_runtime().unwrap_or_else(|err| panic!("Failed to create {name} runtime: {err}"))
        });

        let runtime_ptr: *const VerilatorRuntime = runtime;
        let runtime_ref: &'static VerilatorRuntime = unsafe {
            // SAFETY: The runtime is stored in thread-local storage, so it remains alive for the
            // lifetime of the current test worker thread. Models returned by these helpers are
            // created and dropped entirely within the same thread before thread-local teardown.
            &*runtime_ptr
        };

        f(runtime_ref)
    })
}

macro_rules! cached_model_fn {
    ($fn_name:ident, $model_ty:ident, $create_fn:ident, $name:literal) => {
        pub fn $fn_name() -> riscv_core::$model_ty<'static> {
            thread_local! {
                static RUNTIME: OnceCell<VerilatorRuntime> = const { OnceCell::new() };
            }

            with_cached_runtime(&RUNTIME, riscv_core::$create_fn, $name, |runtime| {
                runtime
                    .create_model_simple::<riscv_core::$model_ty>()
                    .unwrap_or_else(|err| panic!("Failed to create {} model: {}", $name, err))
            })
        }
    };
}

cached_model_fn!(create_alu_model, Alu, create_alu_runtime, "ALU");
cached_model_fn!(create_fpu_model, Fpu, create_fpu_runtime, "FPU");
cached_model_fn!(
    create_fpu_classifier_model,
    FpuClassifier,
    create_fpu_classifier_runtime,
    "FPU classifier"
);
cached_model_fn!(
    create_fpu_comparator_model,
    FpuComparator,
    create_fpu_comparator_runtime,
    "FPU comparator"
);
cached_model_fn!(
    create_fpu_float_to_int_model,
    FpuFloatToInt,
    create_fpu_float_to_int_runtime,
    "FPU float-to-int"
);
cached_model_fn!(
    create_fpu_int_to_float_model,
    FpuIntToFloat,
    create_fpu_int_to_float_runtime,
    "FPU int-to-float"
);
cached_model_fn!(
    create_fpu_sqrt_model,
    FpuSqrt,
    create_fpu_sqrt_runtime,
    "FPU sqrt"
);
cached_model_fn!(
    create_system_controller_model,
    SystemController,
    create_system_controller_runtime,
    "system controller"
);
