//! The desktop's appearance, followed live: one watch of the shared `quire/appearance.toml` and of
//! the settings portal, feeding every window the program opens. The file is the desktop's, not
//! this program's: it is read and watched here, never written, and there is no file of its own.

use anyview_ui::{Look, LookFeed};
use ds::base::spawner::Spawner;
use ds_settings::{
    AppearanceFile, Environment, Loaded, SettingsDoc, Store, SystemPrefsSource, SystemPrefsWatch,
    Watch,
};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::watch;

/// The look every window follows, and the means to hand it to a window.
#[derive(Debug, Clone)]
pub struct Appearances {
    receiver: watch::Receiver<Look>,
}

impl Appearances {
    /// Load the appearance `store` holds and the desktop's preferences from `system`, and keep both
    /// live: the watches run on `spawner`, and every settled change reaches the windows.
    pub async fn follow(
        store: Store,
        system: SystemPrefsSource,
        spawner: Arc<dyn Spawner>,
    ) -> Appearances {
        let loaded = store.load::<AppearanceFile>();
        report(&loaded);
        let prefs = SystemPrefsWatch::start(&system, &*spawner).await;
        let environment = Environment {
            settings: loaded.value,
            system: prefs.current(),
        };
        let (sender, receiver) = watch::channel(look_of(&environment));
        let shared = Arc::new(Shared {
            environment: Mutex::new(environment),
            sender,
        });
        spawner.spawn(Box::pin(follow_files(
            Arc::clone(&shared),
            store.watch::<AppearanceFile>(&*spawner),
        )));
        spawner.spawn(Box::pin(follow_prefs(shared, prefs)));
        Appearances { receiver }
    }

    /// A look that never changes: for a window with no desktop to follow.
    pub fn fixed(look: Look) -> Appearances {
        let (_sender, receiver) = watch::channel(look);
        Appearances { receiver }
    }

    /// The look now.
    pub fn current(&self) -> Look {
        self.receiver.borrow().clone()
    }

    /// The feed a window follows.
    pub fn feed(&self) -> LookFeed {
        LookFeed(self.receiver.clone())
    }
}

/// The environment the watches change, and where the resulting look goes.
struct Shared {
    environment: Mutex<Environment>,
    sender: watch::Sender<Look>,
}

impl Shared {
    /// Change the environment and tell the windows when the look it makes is a new one.
    fn change(&self, change: impl FnOnce(&mut Environment)) {
        let mut environment = self
            .environment
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        change(&mut environment);
        let look = look_of(&environment);
        self.sender.send_if_modified(|current| {
            let new = *current != look;
            if new {
                *current = look;
            }
            new
        });
    }
}

/// What a window draws for `environment`: the choices `Ds` resolves, from the file's keys and the
/// desktop's preferences.
pub fn look_of(environment: &Environment) -> Look {
    let settings = &environment.settings.appearance;
    Look {
        appearance: settings.appearance(),
        system: environment.system,
        tint_alpha: Some(environment.tint_alpha()),
        typeface: Some(settings.typeface()),
        stack: Some(environment.material_stack()),
    }
}

/// Print what the file held that did not become a setting, once per read.
fn report(loaded: &Loaded<AppearanceFile>) {
    for line in loaded.diagnostics(AppearanceFile::FILE) {
        eprintln!("anyview: appearance: {line}");
    }
}

async fn follow_files(shared: Arc<Shared>, mut files: Watch<AppearanceFile>) {
    while let Some(loaded) = files.changed().await {
        report(&loaded);
        shared.change(|environment| environment.settings = loaded.value);
    }
}

async fn follow_prefs(shared: Arc<Shared>, mut prefs: SystemPrefsWatch) {
    while let Some(system) = prefs.changed().await {
        shared.change(|environment| environment.system = system);
    }
}

#[cfg(test)]
mod tests;
