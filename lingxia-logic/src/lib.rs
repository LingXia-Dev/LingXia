use lingxia_lxapp::lx::{LxLogicExtension, register_logic_extension};
use rong::{JSContext, JSResult};

mod env;
mod navigator;
mod system;
mod ui;

pub struct LxLogicRuntime;

impl LxLogicExtension for LxLogicRuntime {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        env::init(ctx)?;
        system::init(ctx)?;
        navigator::init(ctx)?;
        ui::init(ctx)?;
        Ok(())
    }
}

pub fn register_logic_runtime() {
    register_logic_extension(Box::new(LxLogicRuntime));
}
