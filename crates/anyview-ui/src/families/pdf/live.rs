//! What the window knows of an open PDF that is not the stage machine's: the tiles on the GPU, the
//! batches asked for and not yet answered, the hits of the search, the thumbnails and the links of
//! the pages seen. The stage machine holds where the reader is and which hit is current; this holds
//! the things those point at. Every change is a method here, so the whole of it is tested without a
//! window, and the window only reads it and hands it what a worker answered.

use super::cache::{BUDGET, Slot, TileCache};
use super::work::{Finish, FlightId, PdfAnswer};
use crate::io::Stop;
use crate::{
    Destination, FindOut, HitCount, HitIndex, PageView, PdfIn, PdfOut, PdfStage, Stage, StageIn,
    TypedText,
};
use anyview_core::PageIndex;
use anyview_pdf::{Hits, PdfLink, Schedule, TileBatch, TileCoord, TileKey};
use ds_blitz::TextureHandle;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The most tiles one job draws: a short job puts its tiles on screen sooner and is cut short at
/// less cost.
const TILES_PER_JOB: usize = 6;

/// A batch asked for and not yet answered.
#[derive(Debug)]
struct Flight {
    keys: Vec<TileKey>,
    stop: Stop,
}

/// A batch the window is to hand to a worker.
#[derive(Debug, Clone)]
pub(super) struct Asked {
    pub flight: FlightId,
    pub batch: TileBatch,
    pub stop: Stop,
}

/// What the machine's outputs ask the view to do the next time it can.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Wants {
    /// Search for this.
    pub search: Option<TypedText>,
    /// Show this hit.
    pub show: Option<HitIndex>,
    /// Scroll here.
    pub scroll: Option<Destination>,
}

/// The state of the document's search.
#[derive(Debug, Default)]
struct Search {
    asked: Option<TypedText>,
    stop: Option<Stop>,
    hits: Hits,
}

/// The window's side of an open PDF.
#[derive(Debug, Default)]
pub(super) struct PdfLive {
    tiles: TileCache,
    flights: HashMap<FlightId, Flight>,
    flown: u64,
    failed: HashSet<TileKey>,
    search: Search,
    pub wants: Wants,
    thumbs: HashMap<PageIndex, TextureHandle>,
    thumbs_asked: HashSet<PageIndex>,
    links: HashMap<PageIndex, Arc<Vec<PdfLink>>>,
    links_asked: HashSet<PageIndex>,
}

impl PdfLive {
    pub(super) fn tiles(&self) -> &TileCache {
        &self.tiles
    }

    pub(super) fn hits(&self) -> &Hits {
        &self.search.hits
    }

    pub(super) fn thumb(&self, page: PageIndex) -> Option<&TextureHandle> {
        self.thumbs.get(&page)
    }

    pub(super) fn links(&self, page: PageIndex) -> Option<&Arc<Vec<PdfLink>>> {
        self.links.get(&page)
    }

    /// Settles the tiles for the room: ends the batches nothing wants any more, lets go of tiles
    /// over budget, and answers the batches still to ask for, the nearest first.
    pub(super) fn plan(&mut self, schedule: &Schedule, reader: PageIndex) -> Vec<Asked> {
        self.flights.retain(|_, flight| {
            let wanted = flight.keys.iter().any(|key| schedule.wants(key));
            if !wanted {
                flight.stop.request();
            }
            wanted
        });
        self.tiles
            .trim(BUDGET, schedule.zoom(), reader, &|key| schedule.wants(key));
        let flying: HashSet<TileKey> = self
            .flights
            .values()
            .flat_map(|flight| flight.keys.iter().copied())
            .collect();
        let lacking = schedule.missing(|key| {
            self.tiles.contains(key) || flying.contains(key) || self.failed.contains(key)
        });
        let mut asked = Vec::new();
        for batch in lacking.batches() {
            for tiles in batch.tiles.chunks(TILES_PER_JOB) {
                asked.push(self.fly(&batch, tiles));
            }
        }
        asked
    }

    fn fly(&mut self, batch: &TileBatch, tiles: &[TileCoord]) -> Asked {
        self.flown += 1;
        let flight = FlightId(self.flown);
        let batch = TileBatch {
            tiles: tiles.to_vec(),
            ..batch.clone()
        };
        let keys = tiles
            .iter()
            .map(|at| TileKey {
                page: batch.page,
                zoom: batch.zoom,
                x: at.x,
                y: at.y,
            })
            .collect();
        let stop = Stop::new();
        self.flights.insert(
            flight,
            Flight {
                keys,
                stop: stop.clone(),
            },
        );
        Asked {
            flight,
            batch,
            stop,
        }
    }

    /// Notes that a search for `query` is being asked for, ending the one before it.
    pub(super) fn searching(&mut self, query: TypedText) -> Stop {
        if let Some(before) = self.search.stop.take() {
            before.request();
        }
        let stop = Stop::new();
        self.search = Search {
            asked: Some(query),
            stop: Some(stop.clone()),
            hits: Hits::default(),
        };
        stop
    }

    /// Whether a thumbnail of `page` has been asked for; asking is noted by this call.
    pub(super) fn thumb_wanted(&mut self, page: PageIndex) -> bool {
        !self.thumbs.contains_key(&page) && self.thumbs_asked.insert(page)
    }

    /// Whether the links of `page` have been asked for; asking is noted by this call.
    pub(super) fn links_wanted(&mut self, page: PageIndex) -> bool {
        !self.links.contains_key(&page) && self.links_asked.insert(page)
    }

    /// What a worker answered. The machine input it calls for, if any: the search's answer is
    /// one, with the hit nearest the reader's page in `stage`.
    pub(super) fn received(&mut self, answer: PdfAnswer, stage: &Stage) -> Option<StageIn> {
        match answer {
            PdfAnswer::Tiles {
                flight,
                ready,
                unfinished: _,
                failed,
            } => {
                self.flights.remove(&flight);
                for tile in ready {
                    self.tiles.insert(
                        tile.key,
                        Slot {
                            texture: tile.texture,
                            size: tile.size,
                        },
                    );
                }
                self.failed.extend(failed);
                None
            }
            PdfAnswer::Searched { query, hits, end } => self.searched(query, hits, end, stage),
            PdfAnswer::Thumb { page, texture } => {
                match texture {
                    Some(texture) => {
                        self.thumbs.insert(page, texture);
                    }
                    None => {
                        self.thumbs_asked.remove(&page);
                    }
                }
                None
            }
            PdfAnswer::Links { page, links } => {
                self.links.insert(page, Arc::new(links));
                None
            }
        }
    }

    fn searched(
        &mut self,
        query: TypedText,
        hits: Hits,
        end: Finish,
        stage: &Stage,
    ) -> Option<StageIn> {
        if end == Finish::Cut || self.search.asked.as_ref() != Some(&query) {
            return None;
        }
        let count = HitCount(u32::try_from(hits.len()).unwrap_or(u32::MAX));
        let nearest = HitIndex(hits.nearest(reader_page(stage)));
        self.search.hits = hits;
        Some(StageIn::Pdf(PdfIn::Results {
            query,
            count,
            nearest,
        }))
    }

    /// What an output of the stage machine asks of the view.
    pub(super) fn carry(&mut self, out: PdfOut) {
        match out {
            PdfOut::ScrollTo(place) => self.wants.scroll = Some(place),
            PdfOut::Find(FindOut::Search(query)) => self.wants.search = Some(query),
            PdfOut::Find(FindOut::ShowHit(hit)) => self.wants.show = Some(hit),
            PdfOut::Find(FindOut::Clear) => {
                if let Some(stop) = self.search.stop.take() {
                    stop.request();
                }
                self.search = Search::default();
                self.wants.search = None;
                self.wants.show = None;
            }
            PdfOut::Remember(_) => {}
        }
    }
}

/// Where the reader is, in whichever state the PDF stage is; `None` for any other stage.
pub(super) fn page_view(stage: &Stage) -> Option<PageView> {
    match stage {
        Stage::Pdf(
            PdfStage::Reading { view }
            | PdfStage::Finding { view, .. }
            | PdfStage::Jumping { view, .. },
        ) => Some(*view),
        Stage::NoStage | Stage::Raster(_) | Stage::Media(_) | Stage::Text(_) => None,
    }
}

/// The page the reader is on.
pub(super) fn reader_page(stage: &Stage) -> PageIndex {
    page_view(stage).map_or(PageIndex(0), |view| view.page)
}
