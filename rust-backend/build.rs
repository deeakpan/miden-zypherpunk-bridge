use std::{env, fs, path::Path, sync::Arc};
use miden_lib::transaction::TransactionKernel;
use miden_objects::{
    assembly::{Assembler, DefaultSourceManager, LibraryPath, Module, ModuleKind},
    utils::Serializable,
};

fn main() {
    // Rebuild when MASM files change
    println!("cargo:rerun-if-changed=src/asm");

    let out_dir = env::var("OUT_DIR").unwrap();
    let contracts_dir = Path::new("src/asm/contracts");
    let note_scripts_dir = Path::new("src/asm/note_scripts");
    let assets_dir = Path::new(&out_dir).join("assets/note_scripts");
    
    // Create assets directory
    fs::create_dir_all(&assets_dir).unwrap();

    // Start with base assembler
    let mut assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Compile fungible_wrapper contract as a library and add it to assembler
    let fungible_wrapper_path = contracts_dir.join("fungible_wrapper.masm");
    if fungible_wrapper_path.exists() {
        let code = fs::read_to_string(&fungible_wrapper_path)
            .expect("Failed to read fungible_wrapper.masm");
        
        let source_manager = Arc::new(DefaultSourceManager::default());
        let library_path = LibraryPath::new("bridge::fungible_wrapper")
            .expect("Invalid library path");
        let module = Module::parser(ModuleKind::Library)
            .parse_str(
                library_path,
                &code,
                &source_manager,
            )
            .expect("Failed to parse fungible_wrapper module");
        
        let library = assembler
            .clone()
            .assemble_library([module])
            .expect("Failed to assemble fungible_wrapper library");
        
        assembler = assembler
            .with_dynamic_library(library)
            .expect("Failed to add fungible_wrapper library to assembler");
    }

    // Compile note scripts with the assembler that has fungible_wrapper available
    if let Ok(entries) = fs::read_dir(note_scripts_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("masm") {
                    compile_note_script(&path, &assets_dir, assembler.clone());
                }
            }
        }
    }
}

fn compile_note_script(masm_path: &Path, output_dir: &Path, assembler: Assembler) {
    match assembler.assemble_program(masm_path) {
        Ok(program) => {
            let bytes = program.to_bytes();
            let masb_name = masm_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let masb_path = output_dir.join(format!("{}.masb", masb_name));
            fs::write(&masb_path, bytes).unwrap();
            println!("cargo:warning=Compiled {} -> {}", masm_path.display(), masb_path.display());
        }
        Err(e) => {
            panic!("Failed to compile {}: {:?}", masm_path.display(), e);
        }
    }
}

