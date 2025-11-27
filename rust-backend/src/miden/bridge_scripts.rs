use miden_objects::{
    note::NoteScript,
    utils::{sync::LazyLock, Deserializable},
    vm::Program,
};

static CROSSCHAIN_SCRIPT: LazyLock<NoteScript> = LazyLock::new(|| {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/assets/note_scripts/CROSSCHAIN.masb"));
    let program =
        Program::read_from_bytes(bytes).expect("Shipped CROSSCHAIN script is well-formed");
    NoteScript::new(program)
});

pub fn crosschain() -> NoteScript {
    CROSSCHAIN_SCRIPT.clone()
}

