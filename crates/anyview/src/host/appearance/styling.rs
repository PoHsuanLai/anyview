//! The sheets above the viewer's own in the cascade (quire customization design section 7): the
//! chosen look Extension's, the person's desktop-wide `style.css` and their `style.css` for this
//! program, and the crash guard that turns them off for a start that follows two starts which
//! never got going. One value per program, read by the host; every window gets the result in its
//! [`Look`](anyview_ui::Look).

use ds::style::kit::{CustomStyles, LookStyle, StyleHomes, Styles, UserStyle};
use ds_blitz::StylesGuard;
use ds_blitz::styles_guard::Load;
use ds_extensions::look::{LookSheet, stylesheet_for};
use ds_extensions::{Capability, ExtensionsFile, Search};
use ds_settings::Store;
use ds_settings::schema::Provided;
use std::path::PathBuf;

/// The name the crash guard keeps this program's record under.
const PROGRAM: &str = "anyview";

/// Where the sheets other than the desktop's come from: the program's own config directory (its
/// `style.css`), the folders the look Extensions are in, and where the crash guard keeps its
/// record. All are values, so a test hands its own scratch directories.
#[derive(Debug, Clone)]
pub struct Styling {
    pub(super) app: Store,
    search: Search,
    state: PathBuf,
}

impl Styling {
    /// The program's styling: its settings `app` (holding `style.css`), the `search` that finds
    /// look Extensions, and the `state` directory of the crash guard (`$XDG_STATE_HOME/quire`).
    #[must_use]
    pub fn new(app: Store, search: Search, state: PathBuf) -> Styling {
        Styling { app, search, state }
    }
}

/// The look's sheet as it was read for the choice that was in force.
#[derive(Debug)]
struct Looked {
    choice: Provided,
    sheet: Option<LookSheet>,
}

/// What the program draws from the sheets, for one look.
#[derive(Debug)]
pub(super) struct Drawn {
    /// The three sheets and the folders they may read files from.
    pub(super) styles: Styles,
    /// Whether they are drawn: `Off` when the person turned them off or the guard did.
    pub(super) custom: CustomStyles,
    /// The guard, when there are sheets and the person has not turned them off.
    pub(super) guard: Option<StylesGuard>,
}

/// The three sheets as last read, and the crash guard over them.
#[derive(Debug)]
pub(super) struct Sheets {
    styling: Styling,
    /// The desktop's settings, where `style.css` and `extensions.toml` are.
    desktop_store: Store,
    /// `~/.config/quire/style.css`, kept current by the host's watch.
    pub(super) desktop: UserStyle,
    /// `~/.config/anyview/style.css`, kept current by the host's watch.
    pub(super) app: UserStyle,
    looked: Option<Looked>,
    guard: Option<StylesGuard>,
}

impl Sheets {
    /// The sheets as the files hold them now. `desktop` is the desktop's settings store.
    pub(super) fn read(styling: Styling, desktop: Store) -> Sheets {
        Sheets {
            desktop: desktop.load::<UserStyle>().value,
            app: styling.app.load::<UserStyle>().value,
            styling,
            desktop_store: desktop,
            looked: None,
            guard: None,
        }
    }

    /// What to draw for the look `choice` and the switch `custom`. The look's sheet is read again
    /// only when `choice` is not the one it was last read for.
    pub(super) fn draw(&mut self, choice: &Provided, custom: CustomStyles) -> Drawn {
        self.keep_look(choice);
        let styles = self.styles();
        let (custom, guard) = self.guarded(&styles, custom);
        Drawn {
            styles,
            custom,
            guard,
        }
    }

    /// The button Turn On Again: forget the starts that failed and begin again with the same sheets.
    pub(super) fn turn_on_again(&mut self) {
        let styles = self.styles();
        self.guard = self.guard.take().map(|guard| guard.turn_on_again(&styles));
    }

    fn styles(&self) -> Styles {
        let sheet = self
            .looked
            .as_ref()
            .and_then(|looked| looked.sheet.as_ref());
        Styles {
            look: LookStyle(sheet.map(|sheet| sheet.text.clone()).unwrap_or_default()),
            desktop: self.desktop.clone(),
            app: self.app.clone(),
            homes: StyleHomes {
                look: sheet.map(|sheet| sheet.folder.clone()),
                desktop: self.desktop_store.dir(),
                app: self.styling.app.dir(),
            },
        }
    }

    fn keep_look(&mut self, choice: &Provided) {
        if self
            .looked
            .as_ref()
            .is_some_and(|looked| looked.choice == *choice)
        {
            return;
        }
        let sheet = self.look_sheet(choice);
        self.looked = Some(Looked {
            choice: choice.clone(),
            sheet,
        });
    }

    /// The chosen look Extension's sheet: the standard look has none, and neither has a choice
    /// that is not installed, is turned off, or whose sheet cannot be read now.
    fn look_sheet(&self, choice: &Provided) -> Option<LookSheet> {
        choice.chosen()?;
        let off = self.desktop_store.load::<ExtensionsFile>().value.off_ids();
        let looks = self.styling.search.scan(&off).resolve(Capability::Look);
        stylesheet_for(choice, &looks.registry)
    }

    /// The crash guard's answer for `styles`: there is none to ask when the person turned the
    /// styles off or there are none, and the first start of these sheets begins a record.
    fn guarded(
        &mut self,
        styles: &Styles,
        custom: CustomStyles,
    ) -> (CustomStyles, Option<StylesGuard>) {
        if custom == CustomStyles::Off {
            return (CustomStyles::Off, None);
        }
        if styles.is_blank() {
            self.guard = None;
            return (CustomStyles::Use, None);
        }
        let guard = match self.guard.take() {
            Some(guard) => guard.changed(styles),
            None => StylesGuard::begin(&self.styling.state, PROGRAM, styles),
        };
        self.guard = Some(guard.clone());
        let custom = match guard.load() {
            Load::WithStyles => CustomStyles::Use,
            Load::Without => CustomStyles::Off,
        };
        (custom, Some(guard))
    }
}
