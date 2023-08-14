use log::info;
use tokio::sync::broadcast::Sender;

use crate::streams::{Event, NewBlock};

pub async fn event_handler(event_sender: Sender<Event>) {
    let mut event_receiver = event_sender.subscribe();

    let mut new_block = NewBlock::default();

    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(block) => {
                    info!("{:?}", block);
                }
                Event::PendingTx(tx) => {
                    info!("{:?}", tx);
                }
            },
            Err(_) => {}
        }
    }
}
