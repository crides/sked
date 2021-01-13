pub mod cmd;
pub mod sched;
pub mod task;
pub mod time;
mod tui;
mod util;

use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use gluon::{
    import::{add_extern_module, add_extern_module_with_deps},
    Result as GluonResult, RootedThread, ThreadExt, VmBuilder,
};

pub fn get_vm(config_dir: PathBuf) -> RootedThread {
    let vm = VmBuilder::new().import_paths(Some(vec![config_dir])).build();
    vm.run_io(true);
    add_extern_module(&vm, "sched.time.prim", time::load);
    add_extern_module(&vm, "sched.cmd.prim", cmd::load);
    add_extern_module(&vm, "sched.tui", tui::load);
    add_extern_module(&vm, "sched.util.prim", util::load);
    add_extern_module_with_deps(
        &vm,
        "sched.base.prim",
        sched::load,
        vec!["std.map".into(), "sched.time.prim".into(), "std.json".into()],
    );
    vm
}

pub fn run_user(vm: &RootedThread, init_file: &Path) -> GluonResult<()> {
    let script = read_to_string(init_file)?;
    vm.load_script(init_file.to_str().unwrap(), &script)?;
    Ok(())
}
