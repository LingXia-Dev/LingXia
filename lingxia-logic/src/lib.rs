use lingxia_lxapp::lx::{LxLogicExtension, register_logic_extension};
use rong::{JSContext, JSResult};

mod device;
mod env;
mod location;
mod media;
mod navigator;
mod open;
mod system;
mod ui;

pub struct LxLogicRuntime;

impl LxLogicExtension for LxLogicRuntime {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        env::init(ctx)?;
        device::init(ctx)?;
        location::init(ctx)?;
        navigator::init(ctx)?;
        ui::init(ctx)?;
        system::init(ctx)?;
        media::init(ctx)?;
        open::init(ctx)?;
        Ok(())
    }
}

pub fn register_logic_runtime() {
    register_logic_extension(Box::new(LxLogicRuntime));
}
