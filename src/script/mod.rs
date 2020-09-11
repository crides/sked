pub mod sched;
pub mod time;

use std::fs::read_to_string;
use std::path::Path;

use gluon::{
    import::{add_extern_module, add_extern_module_with_deps},
    new_vm, Result as GluonResult, RootedThread, ThreadExt,
};

pub struct ScriptContext {
    pub vm: RootedThread,
}

impl ScriptContext {
    pub fn new() -> Self {
        let vm = new_vm();
        vm.run_io(true);
        vm.load_file("std.map").unwrap();
        add_extern_module(&vm, "time", time::load);
        add_extern_module_with_deps(&vm, "sched", sched::load, vec!["time".into()]);
        Self { vm }
    }

    pub fn init_user<P: AsRef<Path>>(&self, config_dir: P) -> GluonResult<()> {
        let config_dir = config_dir.as_ref();
        let init_file = config_dir.join("init.glu");
        let script = read_to_string(&init_file)?;
        self.vm.load_script(init_file.to_str().unwrap(), &script)?;
        Ok(())
    }
}
