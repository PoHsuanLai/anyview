//! A renamed file keeps its history: its versions move to where the new path's would be.

use crate::error::StoreError;
use crate::io;
use crate::versions::{Sidecar, Versions, key_of};
use std::fs;
use std::path::Path;

impl Versions {
    /// Moves the versions kept of the real path `from` to the real path `to`, so Revert To still
    /// lists them after the file is renamed. Returns how many moved. Each is made whole under the
    /// new path before the old one goes, so a crash leaves a version twice and never none.
    pub fn rekey(&self, from: &Path, to: &Path) -> Result<usize, StoreError> {
        let to_key = key_of(to)?;
        let folder = self.root.join(&to_key);
        let mut moved = 0;
        for version in self.versions_of(from)? {
            io::create_private_dir(&self.root)?;
            io::create_private_dir(&folder)?;
            let kept =
                self.claim_slot(&to_key, &version.id.data_path(&self.root), version.saved_at)?;
            let sidecar = Sidecar {
                path: to
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| StoreError::PathNotUtf8 {
                        path: to.to_path_buf(),
                    })?,
                saved_at: version.saved_at,
                size: version.size.0,
            };
            io::write_private_json(&kept.sidecar_path(&self.root), &sidecar)?;
            io::remove(&version.id.sidecar_path(&self.root))?;
            io::remove(&version.id.data_path(&self.root))?;
            moved += 1;
        }
        // Fails while other files' versions remain, which is the answer wanted.
        let _kept = fs::remove_dir(self.root.join(key_of(from)?));
        Ok(moved)
    }
}
