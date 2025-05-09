use rong::{JSContext, JSFunc, JSResult, Source};

mod app;
mod page;

const PAGE_JS: &str = include_str!("../scripts/Page.js");

pub fn init(ctx: &JSContext) -> JSResult<()> {
    // Register the MiniApp and MiniAppPage class
    ctx.register_class::<app::AppSvc>()?;
    ctx.register_class::<page::PageSvc>()?;

    // Register the global App function
    let app_func = JSFunc::new(ctx, app::app_func)?.name("App")?;
    ctx.global().set("App", app_func)?;

    // Register the global Page function
    let page_func = JSFunc::new(ctx, page::page_func)?.name("_Page")?;
    ctx.global().set("_Page", page_func)?;

    let page = Source::from_bytes(PAGE_JS);
    ctx.eval::<()>(page)?;

    Ok(())
}
