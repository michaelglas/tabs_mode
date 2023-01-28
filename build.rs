extern crate bindgen;
extern crate pkg_config;

use std::env;
use std::path::PathBuf;

fn make_cc_options(lib: &pkg_config::Library) -> impl Iterator<Item = String> + '_ {
    std::iter::empty()
        .chain(lib.libs.iter().map(|lib| "-l".to_owned() + lib))
        .chain(lib.link_paths.iter().map(|path| "-L".to_owned() + path.to_str().unwrap()))
        .chain(lib.include_paths.iter().map(|path| "-I".to_owned() + path.to_str().unwrap()))
        .chain(lib.defines.iter().map(|(k, v)| v.as_ref().map_or_else(|| "-D".to_owned() + k, |v| "-D".to_owned() + k + "=" + v)))
}

fn link_library(lib: &pkg_config::Library) -> () {
    for lib_path in &lib.link_paths {
        println!("cargo:rustc-link-search={}", lib_path.to_str().unwrap());
    }
    for lib in &lib.libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
}

#[derive(Debug)]
struct CustomCallbacks<T>(T);

impl<T: bindgen::callbacks::ParseCallbacks>  bindgen::callbacks::ParseCallbacks for CustomCallbacks<T> {
    fn will_parse_macro(&self, _name: &str) -> bindgen::callbacks::MacroParsingBehavior {
        self.0.will_parse_macro(_name)
    }
    fn int_macro(&self, _name: &str, _value: i64) -> Option<bindgen::callbacks::IntKind> {
        self.0.int_macro(_name, _value)
    }
    fn str_macro(&self, _name: &str, _value: &[u8]) {
        self.0.str_macro(_name, _value)
    }
    fn func_macro(&self, _name: &str, _value: &[&[u8]]) {
        self.0.func_macro(_name, _value)
    }
    fn enum_variant_behavior(
        &self,
        _enum_name: Option<&str>,
        _original_variant_name: &str,
        _variant_value: bindgen::callbacks::EnumVariantValue
    ) -> Option<bindgen::callbacks::EnumVariantCustomBehavior> {
        match _original_variant_name {
            "FP_NAN" | "FP_INFINITE" | "FP_ZERO" | "FP_SUBNORMAL" | "FP_NORMAL" => {
                Some(bindgen::callbacks::EnumVariantCustomBehavior::Hide)
            },
            _ => {
                self.0.enum_variant_behavior(_enum_name, _original_variant_name, _variant_value)
            },
        }
    }
    fn enum_variant_name(
        &self,
        _enum_name: Option<&str>,
        _original_variant_name: &str,
        _variant_value: bindgen::callbacks::EnumVariantValue
    ) -> Option<String> {
        self.0.enum_variant_name(_enum_name, _original_variant_name, _variant_value)
    }
    fn item_name(&self, _original_item_name: &str) -> Option<String> {
        self.0.item_name(_original_item_name)
    }
    fn include_file(&self, _filename: &str) {
        self.0.include_file(_filename)
    }
    fn blocklisted_type_implements_trait(
        &self,
        _name: &str,
        _derive_trait: bindgen::callbacks::DeriveTrait
    ) -> Option<bindgen::callbacks::ImplementsTrait> {
        self.0.blocklisted_type_implements_trait(_name, _derive_trait)
    }
    fn add_derives(&self, _name: &str) -> Vec<String> {
        self.0.add_derives(_name)
    }
}

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=src/wrapper.h");

    let wofi = pkg_config::Config::new()
        .atleast_version("v1.2.4")
        .probe("wofi")
        .unwrap();

    let gdk_pixbuf = pkg_config::Config::new()
        .atleast_version("2.42.4")
        .probe("gdk-pixbuf-2.0")
        .unwrap();

    link_library(&wofi);
    link_library(&gdk_pixbuf);


    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("src/wrapper.h")
        .clang_args(make_cc_options(&wofi))
        .clang_args(make_cc_options(&&gdk_pixbuf))
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(CustomCallbacks(bindgen::CargoCallbacks)))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
