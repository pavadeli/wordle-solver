use std::panic;

use color_eyre::{config::HookBuilder, eyre};

use crate::tui;

/// This replaces the standard color_eyre panic and error hooks with hooks that
/// restore the terminal before printing the panic or error.
pub fn install_hooks() -> color_eyre::Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();

    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tui::restore();
        panic_hook(panic_info)
    }));

    let eyre_hook = eyre_hook.into_eyre_hook();
    eyre::set_hook(Box::new(move |error| {
        tui::restore();
        eyre_hook(error)
    }))?;

    Ok(())
}
