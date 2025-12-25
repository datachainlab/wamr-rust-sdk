/*
 * Copyright (C) 2023 Liquid Reply GmbH. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 */

extern crate bindgen;
extern crate cmake;

use cmake::Config;
use std::{env, path::Path, path::PathBuf};

type FeatureFlags = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

const LLVM_LIBRARIES: &[&str] = &[
    // keep alphabet order
    "LLVMOrcJIT",
    "LLVMOrcShared",
    "LLVMOrcTargetProcess",
    "LLVMPasses",
    "LLVMProfileData",
    "LLVMRuntimeDyld",
    "LLVMScalarOpts",
    "LLVMSelectionDAG",
    "LLVMSymbolize",
    "LLVMTarget",
    "LLVMTextAPI",
    "LLVMTransformUtils",
    "LLVMVectorize",
    "LLVMX86AsmParser",
    "LLVMX86CodeGen",
    "LLVMX86Desc",
    "LLVMX86Disassembler",
    "LLVMX86Info",
    "LLVMXRay",
    "LLVMipo",
];

fn check_is_espidf() -> bool {
    let is_espidf = env::var("CARGO_FEATURE_ESP_IDF").is_ok()
        && env::var("CARGO_CFG_TARGET_OS").unwrap() == "espidf";

    if is_espidf
        && (env::var("WAMR_BUILD_PLATFORM").is_ok()
            || env::var("WAMR_SHARED_PLATFORM_CONFIG").is_ok())
    {
        panic!("ESP-IDF build cannot use custom platform build (WAMR_BUILD_PLATFORM) or shared platform config (WAMR_SHARED_PLATFORM_CONFIG)");
    }

    is_espidf
}

fn check_is_sgx() -> bool {
    let is_sgx = cfg!(feature = "sgx")
        || env::var("CARGO_FEATURE_SGX").is_ok()
        || env::var("CARGO_CFG_TARGET_OS").unwrap() == "sgx";
    if is_sgx
        && (env::var("WAMR_BUILD_PLATFORM").is_ok()
            || env::var("WAMR_SHARED_PLATFORM_CONFIG").is_ok())
    {
        panic!("SGX build cannot use custom platform build (WAMR_BUILD_PLATFORM) or shared platform config (WAMR_SHARED_PLATFORM_CONFIG)");
    }

    is_sgx
}

macro_rules! wamr_build_enable_option {
    ($feature:expr) => {
        if cfg!(feature = $feature) { "1" } else { "0" }.to_string()
    };
}

fn get_feature_flags() -> FeatureFlags {
    let enable_llvm_jit = wamr_build_enable_option!("llvmjit");
    if enable_llvm_jit == "1" && !check_is_sgx() {
        println!("cargo:warning=LLVM JIT is enabled, but not on SGX");
    }
    let disable_hw_bound_check = if cfg!(feature = "hw-bound-check") {
        "0"
    } else {
        "1"
    }
    .to_string();

    (
        wamr_build_enable_option!("libc-wasi"),
        wamr_build_enable_option!("libc-builtin"),
        wamr_build_enable_option!("custom-section"),
        wamr_build_enable_option!("dump-call-stack"),
        wamr_build_enable_option!("fast-interp"),
        enable_llvm_jit,
        wamr_build_enable_option!("multi-module"),
        wamr_build_enable_option!("name-section"),
        disable_hw_bound_check,
    )
}

fn link_llvm_libraries(llvm_cfg_path: &String, enable_llvm_jit: &String) {
    if enable_llvm_jit == "0" {
        return;
    }

    let llvm_cfg_path = PathBuf::from(llvm_cfg_path);
    assert!(llvm_cfg_path.exists());

    let llvm_lib_path = llvm_cfg_path.join("../../../lib").canonicalize().unwrap();
    assert!(llvm_lib_path.exists());

    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-link-lib=dylib=rt");
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=z");
    println!("cargo:libdir={}", llvm_lib_path.display());
    println!("cargo:rustc-link-search=native={}", llvm_lib_path.display());

    for &llvm_lib in LLVM_LIBRARIES {
        println!("cargo:rustc-link-lib=static={}", llvm_lib);
    }
}

fn setup_config(cmakelists_dir: &PathBuf, feature_flags: FeatureFlags) -> Config {
    let (
        enable_libc_wasi,
        enable_libc_builtin,
        enable_custom_section,
        enable_dump_call_stack,
        enable_fast_interp,
        enable_llvm_jit,
        enable_multi_module,
        enable_name_section,
        disalbe_hw_bound_check,
    ) = feature_flags;

    let mut cfg = Config::new(cmakelists_dir);
    cfg.define("WAMR_BUILD_AOT", "1")
        .define("WAMR_BUILD_INTERP", "1")
        .define("WAMR_BUILD_FAST_INTERP", &enable_fast_interp)
        .define("WAMR_BUILD_JIT", &enable_llvm_jit)
        .define("WAMR_BUILD_BULK_MEMORY", "1")
        .define("WAMR_BUILD_REF_TYPES", "1")
        .define("WAMR_BUILD_SIMD", "0")
        .define("WAMR_BUILD_LIB_PTHREAD", "0")
        .define("WAMR_BUILD_LIBC_WASI", &enable_libc_wasi)
        .define("WAMR_BUILD_LIBC_BUILTIN", &enable_libc_builtin)
        .define("WAMR_DISABLE_HW_BOUND_CHECK", &disalbe_hw_bound_check)
        .define("WAMR_BUILD_MULTI_MODULE", &enable_multi_module)
        .define("WAMR_BUILD_DUMP_CALL_STACK", &enable_dump_call_stack)
        .define("WAMR_BUILD_CUSTOM_NAME_SECTION", &enable_name_section)
        .define("WAMR_BUILD_LOAD_CUSTOM_SECTION", &enable_custom_section);

    // always assume non-empty strings for these environment variables

    if let Ok(platform_name) = env::var("WAMR_BUILD_PLATFORM") {
        cfg.define("WAMR_BUILD_PLATFORM", &platform_name);
    }

    if let Ok(target_name) = env::var("WAMR_BUILD_TARGET") {
        cfg.define("WAMR_BUILD_TARGET", &target_name);
    }

    if let Ok(platform_config) = env::var("WAMR_SHARED_PLATFORM_CONFIG") {
        cfg.define("SHARED_PLATFORM_CONFIG", &platform_config);
    }

    if let Ok(llvm_cfg_path) = env::var("LLVM_LIB_CFG_PATH") {
        link_llvm_libraries(&llvm_cfg_path, &enable_llvm_jit);
        cfg.define("LLVM_DIR", &llvm_cfg_path);
    }

    // STDIN/STDOUT/STDERR redirect
    if let Ok(bh_vprintf) = env::var("WAMR_BH_VPRINTF") {
        cfg.define("WAMR_BH_VPRINTF", &bh_vprintf);
    }

    cfg
}

fn build_wamr_libraries(wamr_root: &PathBuf) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let vmbuild_path = out_dir.join("vmbuild");

    let feature_flags = get_feature_flags();
    let dst = if check_is_sgx() {
        let current_dir = &env::current_dir().unwrap();
        let mut cfg = setup_config(current_dir, feature_flags);
        println!("cargo:rustc-link-lib=static=vmlib");
        cfg.out_dir(vmbuild_path)
            .build_target("default_target")
            .build()
    } else {
        let mut cfg = setup_config(wamr_root, feature_flags);
        println!("cargo:rustc-link-lib=static=iwasm");
        cfg.out_dir(vmbuild_path).build_target("vmlib").build()
    };
    println!("cargo:rustc-link-search=native={}/build", dst.display());
}

fn build_wamrc(wamr_root: &Path) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let wamrc_build_path = out_dir.join("wamrcbuild");

    let wamr_compiler_path = wamr_root.join("wamr-compiler");
    assert!(wamr_compiler_path.exists());

    Config::new(&wamr_compiler_path)
        .out_dir(wamrc_build_path)
        .define("WAMR_BUILD_WITH_CUSTOM_LLVM", "1")
        .define("LLVM_DIR", env::var("LLVM_LIB_CFG_PATH").expect("LLVM_LIB_CFG_PATH isn't specified in config.toml"))
        .build();
}

fn generate_bindings(wamr_root: &Path) {
    let wamr_header = wamr_root.join("core/iwasm/include/wasm_export.h");
    assert!(wamr_header.exists());

    let bindings = bindgen::Builder::default()
        .ctypes_prefix("::core::ffi")
        .use_core()
        .header(wamr_header.into_os_string().into_string().unwrap())
        .derive_default(true)
        .generate()
        .expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");
}

fn set_sgx_environment() {
    macro_rules! set_var_if_not_set {
        ($var_name:expr, $value:expr) => {
            if env::var($var_name).is_err() {
                env::set_var($var_name, $value);
            }
        };
        () => {
        };
    }
    set_var_if_not_set!("SGX_SDK", "/opt/sgxsdk");
    set_var_if_not_set!("SGX_MODE", "SIM");
    set_var_if_not_set!("SGX_ARCH", "x64");
    set_var_if_not_set!("LD_LIBRARY_PATH", "/opt/sgxsdk/lib64");
    set_var_if_not_set!("PKG_CONFIG_PATH", "/opt/sgxsdk/pkgconfig");
}

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ESP_IDF");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    println!("cargo:rerun-if-env-changed=WAMR_BUILD_PLATFORM");
    println!("cargo:rerun-if-env-changed=WAMR_SHARED_PLATFORM_CONFIG");
    println!("cargo:rerun-if-env-changed=LLVM_LIB_CFG_PATH");
    println!("cargo:rerun-if-env-changed=WAMR_BH_VPRINTF");

    set_sgx_environment();

    let wamr_root = env::current_dir().unwrap();
    let wamr_root = wamr_root.join("wasm-micro-runtime");
    assert!(wamr_root.exists());

    if !check_is_espidf() {
        // because the ESP-IDF build procedure differs from the regular one
        // (build internally by esp-idf-sys),
        build_wamr_libraries(&wamr_root);
        // build_wamrc(&wamr_root);
    }

    generate_bindings(&wamr_root);
}
