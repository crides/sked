pub mod cmd;
pub mod job;
pub mod sched;
pub mod time;

use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use gluon::{
    import::{add_extern_module, add_extern_module_with_deps},
    Result as GluonResult, RootedThread, ThreadExt, VmBuilder,
};

pub fn get_vm(config_dir: PathBuf) -> RootedThread {
    let vm = VmBuilder::new().import_paths(Some(vec![config_dir])).build();
    vm.get_database_mut().set_optimize(false);
    vm.run_io(true);
    add_extern_module(&vm, "time.prim", time::load);
    add_extern_module(&vm, "cmd", cmd::load);
    add_extern_module_with_deps(&vm, "sched", sched::load, vec!["std.map".into(), "time.prim".into()]);
    add_extern_module_with_deps(&vm, "jobs", job::load, vec!["time.prim".into()]);
    vm
}

pub fn run_user(vm: &RootedThread, init_file: &Path) -> GluonResult<()> {
    let script = read_to_string(init_file)?;
    vm.load_script(init_file.to_str().unwrap(), &script)?;
    Ok(())
}
