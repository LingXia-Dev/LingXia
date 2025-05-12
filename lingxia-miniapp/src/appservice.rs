use crate::AppController;
use crate::log::LogLevel;

use rong::{Rong, RongJS};
use std::sync::Arc;

pub(crate) fn init<T: AppController + 'static>(controller: Arc<T>, num: usize) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create worker runtime");

        // Run the worker loop
        rt.block_on(async {
            let _rong = Rong::<RongJS>::builder().with_num_workers(num).build();
            controller.log(LogLevel::Info, "init rong");
        });
    });
}
