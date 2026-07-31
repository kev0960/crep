use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct ReindexSignal {
    pub head_commit_id: String,
}

pub type ReindexSignalSender = UnboundedSender<ReindexSignal>;
pub type ReindexSignalReceiver = UnboundedReceiver<ReindexSignal>;
