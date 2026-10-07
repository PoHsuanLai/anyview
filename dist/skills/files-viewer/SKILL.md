---
name: files-viewer
description: Open files for the person in Viewer, which shows pictures, PDFs, text, audio, video and archives.
---
# Opening files in Viewer

Viewer is where the person looks at a file. You open files in it for them; you do not read their contents through it.

**Open.** Call `anyview.file.open` with the files as its target, one or several, each as an absolute path. Viewer opens them in its window and starts itself if it is not running. Several files open together, so the person can step through them.

**What it does.** It only shows the files. It changes nothing on disk and cannot edit, convert or delete a file. If the person wants a file changed, say that Viewer only shows files.

**What you learn.** The call answers that the files were handed over. It does not return what is in them, so do not describe a file you have not read some other way. Say you opened it and what you opened.

**Paths.** Use the exact path the person gave or a tool returned. If you do not know where a file is, find it first or ask; do not guess a path. A path that does not exist is refused: tell the person, do not retry with a changed name.

**Several.** Open them in one call, not one call per file.
