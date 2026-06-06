use crate::fetch;

use crate::AppState;

use std::sync::Arc;

use std::time::Duration;

use tokio::sync::watch;



pub struct SchedulerHandle {

    interval_tx: watch::Sender<u32>,

}



impl SchedulerHandle {

    pub fn update_interval(&self, minutes: u32) {

        let _ = self.interval_tx.send(minutes.max(5));

    }

}



pub fn start_scheduler(state: Arc<AppState>, initial_interval: u32) -> SchedulerHandle {

    let (interval_tx, mut interval_rx) = watch::channel(initial_interval.max(5));

    let interval_tx_clone = interval_tx.clone();



    tauri::async_runtime::spawn(async move {

        loop {

            let minutes = *interval_rx.borrow();

            let sleep_duration = Duration::from_secs(minutes as u64 * 60);



            tokio::select! {

                _ = tokio::time::sleep(sleep_duration) => {

                    let state_clone = state.clone();

                    tauri::async_runtime::spawn(async move {

                        if let Err(e) = fetch::do_fetch(&state_clone).await {

                            eprintln!("Scheduled fetch error: {}", e);

                        }

                    });

                }

                result = interval_rx.changed() => {

                    if result.is_err() {

                        break;

                    }

                }

            }

        }

    });



    SchedulerHandle {

        interval_tx: interval_tx_clone,

    }

}


