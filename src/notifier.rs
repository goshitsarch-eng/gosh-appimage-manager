// Gosh AppImage Manager — update notifier (ports UpdateNotifier).
// Background checks notify only; they never download or apply updates.

use crate::types::UpdateOffer;

pub struct UpdateNotifier {
    app_name: String,
}

impl Default for UpdateNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateNotifier {
    pub fn new() -> Self {
        Self {
            app_name: "Gosh AppImage Manager".to_string(),
        }
    }

    /// Best-effort desktop notification; failures are ignored by design.
    pub fn notify_updates_available(&self, count: usize) {
        let summary = format!("{count} AppImage update(s) available");
        let _ = notify_rust::Notification::new()
            .summary(&self.app_name)
            .body(&summary)
            .appname(&self.app_name)
            .show();
    }

    pub fn notify_offers(&self, offers: &[UpdateOffer]) {
        if !offers.is_empty() {
            self.notify_updates_available(offers.len());
        }
    }
}
