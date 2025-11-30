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

    // STEP 1: Compile fungible_wrapper with placeholder (0) code commitment for compilation
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

    // STEP 2: Compile CROSSCHAIN script (uses fungible_wrapper from assembler)
    let crosschain_path = note_scripts_dir.join("CROSSCHAIN.masm");
    let mut crosschain_code_commitment = None;
    
    if crosschain_path.exists() {
        match assembler.clone().assemble_program(crosschain_path.as_path()) {
            Ok(program) => {
                // Get code commitment (hash of the program)
                let code_commitment = program.hash();
                let commitment_elements = code_commitment.as_elements();
                crosschain_code_commitment = Some([
                    commitment_elements[0].as_int(),
                    commitment_elements[1].as_int(),
                    commitment_elements[2].as_int(),
                    commitment_elements[3].as_int(),
                ]);
                
                // Save compiled script
                let bytes = program.to_bytes();
                let masb_path = assets_dir.join("CROSSCHAIN.masb");
                fs::write(&masb_path, bytes).unwrap();
                println!("cargo:warning=Compiled {} -> {}", crosschain_path.display(), masb_path.display());
                println!("cargo:warning=CROSSCHAIN code commitment: [{}, {}, {}, {}]", 
                    commitment_elements[0].as_int(),
                    commitment_elements[1].as_int(),
                    commitment_elements[2].as_int(),
                    commitment_elements[3].as_int());
            }
            Err(e) => {
                panic!("Failed to compile CROSSCHAIN.masm: {:?}", e);
            }
        }
    }
    
    // STEP 3: Recompile fungible_wrapper with actual CROSSCHAIN code commitment for runtime
    if let Some(commitment) = crosschain_code_commitment {
        let mut code = fs::read_to_string(&fungible_wrapper_path)
            .expect("Failed to read fungible_wrapper.masm");
        
        // Update code commitment
        code = code.replace(
            "const.BRIDGE_NOTE_CODE_COMMITMENT_FELT1=0",
            &format!("const.BRIDGE_NOTE_CODE_COMMITMENT_FELT1={}", commitment[0])
        );
        code = code.replace(
            "const.BRIDGE_NOTE_CODE_COMMITMENT_FELT2=0",
            &format!("const.BRIDGE_NOTE_CODE_COMMITMENT_FELT2={}", commitment[1])
        );
        code = code.replace(
            "const.BRIDGE_NOTE_CODE_COMMITMENT_FELT3=0",
            &format!("const.BRIDGE_NOTE_CODE_COMMITMENT_FELT3={}", commitment[2])
        );
        code = code.replace(
            "const.BRIDGE_NOTE_CODE_COMMITMENT_FELT4=0",
            &format!("const.BRIDGE_NOTE_CODE_COMMITMENT_FELT4={}", commitment[3])
        );
        
        // Create new assembler for runtime version (without the old library)
        let runtime_assembler = TransactionKernel::assembler().with_debug_mode(true);
        
        let source_manager = Arc::new(DefaultSourceManager::default());
        let library_path = LibraryPath::new("bridge::fungible_wrapper")
            .expect("Invalid library path");
        let module = Module::parser(ModuleKind::Library)
            .parse_str(
                library_path,
                &code,
                &source_manager,
            )
            .expect("Failed to parse updated fungible_wrapper module");
        
        let runtime_library = runtime_assembler
            .assemble_library([module])
            .expect("Failed to assemble runtime fungible_wrapper library");
        
        // Save runtime library as .masl file for account component
        let contracts_assets_dir = Path::new(&out_dir).join("assets/contracts");
        fs::create_dir_all(&contracts_assets_dir).unwrap();
        let masl_path = contracts_assets_dir.join("fungible_wrapper.masl");
        let library_bytes = runtime_library.to_bytes();
        fs::write(&masl_path, library_bytes).unwrap();
        println!("cargo:warning=Saved runtime library with CROSSCHAIN code commitment -> {}", masl_path.display());
    } else {
        // Fallback: warn if CROSSCHAIN code commitment not computed
        println!("cargo:warning=WARNING: CROSSCHAIN code commitment not computed, using placeholder values");
    }
    
    // STEP 4: Compile other note scripts (if any) with fungible_wrapper available
    if let Ok(entries) = fs::read_dir(note_scripts_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("masm") {
                    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if file_name != "CROSSCHAIN.masm" {
                        compile_note_script(&path, &assets_dir, assembler.clone());
                    }
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
