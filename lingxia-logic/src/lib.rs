use lxapp::lx::{LxLogicExtension, register_logic_extension};
use rong::{JSContext, JSResult};

mod device;
mod display;
mod env;
mod fs;
include!(concat!(env!("OUT_DIR"), "/i18n_generated.rs"));
pub mod i18n;
mod location;
mod media;
mod navigator;
mod storage;
mod system;
mod ui;
mod update;

pub struct LxLogicRuntime;

impl LxLogicExtension for LxLogicRuntime {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        env::init(ctx)?;
        device::init(ctx)?;
        display::init(ctx)?;
        location::init(ctx)?;
        navigator::init(ctx)?;
        update::init(ctx)?;
        ui::init(ctx)?;
        system::init(ctx)?;
        media::init(ctx)?;
        fs::init(ctx)?;
        storage::init(ctx)?;
        Ok(())
    }
}

pub fn register_logic_runtime() {
    register_logic_extension(Box::new(LxLogicRuntime));
}
