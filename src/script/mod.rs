pub mod sched;
pub mod time;
pub mod job;

use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use gluon::{
    import::{add_extern_module, add_extern_module_with_deps},
    VmBuilder,
    Result as GluonResult, RootedThread, ThreadExt,
};

pub fn get_vm(config_dir: PathBuf) -> RootedThread {
    let vm = VmBuilder::new().import_paths(Some(vec![config_dir])).build();
    vm.run_io(true);
    add_extern_module(&vm, "time.prim", time::load);
    add_extern_module_with_deps(&vm, "sched", sched::load, vec!["std.map".into(), "time.prim".into()]);
    add_extern_module_with_deps(&vm, "jobs", job::load, vec!["time.prim".into()]);
    vm
}

pub fn run_user(vm: &RootedThread, init_file: &Path) -> GluonResult<()> {
    let script = read_to_string(init_file)?;
    vm.load_script(init_file.to_str().unwrap(), &script)?;
    Ok(())
}
