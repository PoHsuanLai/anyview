//! The desktop's appearance, followed live: one watch of the shared `quire/appearance.toml` and of
//! the settings portal, feeding every window the program opens. The file is the desktop's, not
//! this program's: it is read and watched here, never written, and there is no file of its own.

use anyview_ui::{Look, LookFeed, LookSender, StylesRescue};
use ds::base::spawner::Spawner;
use ds::style::kit::UserStyle;
use ds_settings::{
    AppearanceFile, Environment, Loaded, SettingsDoc, Store, SystemPrefsSource, SystemPrefsWatch,
    Watch,
};
use std::sync::{Arc, Mutex, PoisonError, Weak};

mod styling;

use styling::Sheets;
pub use styling::Styling;

/// The look every window follows, and the means to hand it to a window.
#[derive(Debug, Clone)]
pub struct Appearances {
    feed: LookFeed,
}

impl Appearances {
    /// Load the appearance `store` holds and the desktop's preferences from `system`, and keep both
    /// live: the watches run on `spawner`, and every settled change reaches the windows. The
    /// look carries the sheets of `styling` too: `store`'s `style.css`, the program's own and the
    /// chosen look Extension's. The two `style.css` files are watched like the appearance file.
    /// The look Extension's sheet is read when the choice changes, not when its file does.
    pub async fn follow(
        store: Store,
        styling: Styling,
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
        let (sender, feed) = LookFeed::new(look_of(&environment));
        let sheets = Sheets::read(styling.clone(), store.clone());
        let shared = Arc::new_cyclic(|me: &Weak<Shared>| {
            let me = me.clone();
            Shared {
                state: Mutex::new(State {
                    environment,
                    sheets,
                }),
                sender,
                turn_on: Arc::new(move || {
                    if let Some(shared) = me.upgrade() {
                        shared.change(|state| state.sheets.turn_on_again());
                    }
                }),
            }
        });
        // The first look carries the sheets, and the guard begins its record.
        shared.change(|_| {});
        spawner.spawn(Box::pin(follow_files(
            Arc::clone(&shared),
            store.watch::<AppearanceFile>(&*spawner),
        )));
        spawner.spawn(Box::pin(follow_sheet(
            Arc::clone(&shared),
            store.watch::<UserStyle>(&*spawner),
            |sheets| &mut sheets.desktop,
        )));
        spawner.spawn(Box::pin(follow_sheet(
            Arc::clone(&shared),
            styling.app.watch::<UserStyle>(&*spawner),
            |sheets| &mut sheets.app,
        )));
        spawner.spawn(Box::pin(follow_prefs(shared, prefs)));
        Appearances { feed }
    }

    /// A look that never changes: for a window with no desktop to follow.
    #[must_use]
    pub fn fixed(look: Look) -> Appearances {
        let (_sender, feed) = LookFeed::new(look);
        Appearances { feed }
    }

    /// The look now.
    #[must_use]
    pub fn current(&self) -> Look {
        self.feed.current()
    }

    /// The feed a window follows.
    #[must_use]
    pub fn feed(&self) -> LookFeed {
        self.feed.clone()
    }
}

/// The environment and the sheets the watches change, and where the resulting look goes.
struct Shared {
    state: Mutex<State>,
    sender: LookSender,
    /// The button Turn On Again of every window's rescue: one closure, so equal looks stay equal.
    turn_on: Arc<dyn Fn() + Send + Sync>,
}

struct State {
    environment: Environment,
    sheets: Sheets,
}

impl Shared {
    /// Change the state and tell the windows when the look it makes is a new one.
    fn change(&self, change: impl FnOnce(&mut State)) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut state);
        let look = state.look(&self.turn_on);
        self.sender.set(look);
    }
}

impl State {
    /// What a window draws: the environment's look with the sheets, drawn or not as the person's
    /// switch and the crash guard say.
    fn look(&mut self, turn_on: &Arc<dyn Fn() + Send + Sync>) -> Look {
        let settings = &self.environment.settings.appearance;
        let (choice, custom) = (settings.look.clone(), settings.custom_styles);
        let drawn = self.sheets.draw(&choice, custom);
        Look {
            styles: drawn.styles,
            custom_styles: drawn.custom,
            rescue: drawn
                .guard
                .map(|guard| StylesRescue::new(guard, Arc::clone(turn_on))),
            ..look_of(&self.environment)
        }
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
        log::warn!("appearance: {line}");
    }
}

async fn follow_files(shared: Arc<Shared>, mut files: Watch<AppearanceFile>) {
    while let Some(loaded) = files.changed().await {
        report(&loaded);
        shared.change(|state| state.environment.settings = loaded.value);
    }
}

/// Follow one `style.css`: `sheet` picks which of the two the file is.
async fn follow_sheet(
    shared: Arc<Shared>,
    mut files: Watch<UserStyle>,
    sheet: fn(&mut Sheets) -> &mut UserStyle,
) {
    while let Some(loaded) = files.changed().await {
        shared.change(|state| *sheet(&mut state.sheets) = loaded.value);
    }
}

async fn follow_prefs(shared: Arc<Shared>, mut prefs: SystemPrefsWatch) {
    while let Some(system) = prefs.changed().await {
        shared.change(|state| state.environment.system = system);
    }
}

#[cfg(test)]
mod tests;
