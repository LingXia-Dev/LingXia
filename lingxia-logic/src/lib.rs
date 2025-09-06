use lingxia_lxapp::lx::{LxLogicExtension, register_logic_extension};
use rong::{JSContext, JSResult};

mod env;

pub struct LxLogicRuntime;

impl LxLogicExtension for LxLogicRuntime {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        env::init(ctx)?;
        Ok(())
    }
}

pub fn register_logic_runtime() {
    register_logic_extension(Box::new(LxLogicRuntime));
}
