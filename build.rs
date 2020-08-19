extern crate bindgen;

#[cfg(feature = "rpi")]
use std::env;
#[cfg(feature = "rpi")]
use std::path::PathBuf;

#[cfg(not(feature = "rpi"))]
fn main() {}

#[cfg(feature = "rpi")]
fn main() {
    let bindings = bindgen::Builder::default() //builder()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.hpp")
        // The types and functions we want bindings for
        .whitelist_function("new_usrp")
        .whitelist_function("set_clock_source")
        .whitelist_function("set_rx_gain")
        .whitelist_function("get_rx_gain")
        .whitelist_function("set_tx_freq")
        .whitelist_function("set_rx_freq")
        .whitelist_function("set_time_now")
        .whitelist_function("get_rx_streamer")
        .whitelist_function("get_tx_streamer")
        .whitelist_function("recv")
        .whitelist_function("send")
        .whitelist_function("delete_usrp")
        .whitelist_function("delete_rx_stream")
        .whitelist_function("delete_tx_stream")
        .opaque_type("MultiUsrp")
        .opaque_type("RxStream")
        .opaque_type("TxStream")
        .opaque_type("boost::*")
        .opaque_type("std::*")
        // Flags to pass to clang parser
        .clang_arg("-std=c++14")
        .clang_arg("-I/opt/local/include/")
        .clang_arg("-I/usr/include/clang/3.8.1/include/")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Compile the USRP wrapper
    cc::Build::new()
        .file("wrapper.cpp")
        .cpp(true)
        .flag("-pthread")
        .flag("-fPIC")
        .flag("-Wno-write-strings")
        .flag("-std=c++0x")
        .include("/opt/local/include")
        .shared_flag(false)
        .compile("wrapper");

    // Select the files that should be monitored for changes. Only if those files are changed will
    // the build scropt be rerun.
    println!("cargo:rerun-if-changed=wrapper.cpp");
    println!("cargo:rerun-if-changed=wrapper.hpp");

    println!("cargo:rustc-link-lib=uhd");
    println!("cargo:rustc-link-lib=boost_system");
    println!("cargo:rustc-link-search=native=/opt/local/lib/");
    println!("cargo:rustc-link-search=native=..");

    // OS specific libraries
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=boost_thread");

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=boost_thread-mt");
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-flags=-l dylib=c++");
}
