"""Choosing recordings without typing paths.

Opens native dialogs through Tk, which ships with Python. On a machine
without a display (a server, some remote notebooks) it falls back to asking
for the path in the notebook.
"""

from __future__ import annotations

import os
from pathlib import Path

from .io import Recording, list_recordings, load_recording

_last_dir: Path | None = None


def default_data_dir() -> Path:
    """Where the desktop application saves recordings unless told otherwise.

    Override with the ``THRUSTBENCH_DATA`` environment variable when the
    application was configured to use another folder.
    """
    override = os.environ.get("THRUSTBENCH_DATA")
    if override:
        return Path(override)
    return Path.home() / "Documents" / "ThrustBench"


def _start_dir() -> Path:
    start = _last_dir or default_data_dir()
    return start if start.exists() else Path.home()


def _tk_root():
    import tkinter

    root = tkinter.Tk()
    root.withdraw()
    root.attributes("-topmost", True)  # the dialog must not open behind the browser
    return root


def _ask_folder(title: str) -> Path | None:
    try:
        from tkinter import filedialog

        root = _tk_root()
        try:
            picked = filedialog.askdirectory(title=title, initialdir=str(_start_dir()), mustexist=True)
        finally:
            root.destroy()
    except Exception:  # no display, or Tk missing
        picked = input(f"{title}\nFolder path (empty to cancel): ").strip().strip('"')
    return Path(picked) if picked else None


def pick_recording(title: str = "Choose a ThrustBench recording folder") -> Recording:
    """Opens a folder dialog and loads the chosen recording."""
    global _last_dir
    folder = _ask_folder(title)
    if folder is None:
        raise RuntimeError("no recording was chosen")
    _last_dir = folder.parent
    return load_recording(folder)


def _ask_many(root_dir: Path) -> list[Path]:
    """List with multiple selection; folder dialogs only pick one folder."""
    import tkinter

    table = list_recordings(root_dir)
    if table.empty:
        raise RuntimeError(f"no recordings in {root_dir}")
    root = tkinter.Tk()
    root.title("Choose recordings to compare (Ctrl or Shift to select several)")
    root.attributes("-topmost", True)
    listbox = tkinter.Listbox(root, selectmode=tkinter.EXTENDED, width=110, height=min(25, len(table)))
    for row in table.itertuples():
        listbox.insert(tkinter.END, f"{row.started_at[:19]}   {row.duration_s:6.1f} s   {row.motor or '-'}   {row.propeller or '-'}   {row.notes}")
    listbox.pack(padx=10, pady=10, fill=tkinter.BOTH, expand=True)
    chosen: list[int] = []

    def accept() -> None:
        chosen.extend(listbox.curselection())
        root.destroy()

    tkinter.Button(root, text="Compare", command=accept).pack(pady=(0, 10))
    root.mainloop()
    return [Path(table.loc[i, "path"]) for i in chosen]


def pick_recordings(root_dir: str | Path | None = None) -> list[Recording]:
    """Lets the user tick several recordings of a folder and loads them."""
    global _last_dir
    if root_dir is None:
        root_dir = _ask_folder("Choose the folder that holds the recordings") if not default_data_dir().exists() else _start_dir()
        if root_dir is None:
            raise RuntimeError("no folder was chosen")
    root_dir = Path(root_dir)
    try:
        folders = _ask_many(root_dir)
    except RuntimeError:
        raise
    except Exception:  # no display
        table = list_recordings(root_dir)
        print(table[["started_at", "motor", "propeller", "notes"]].to_string())
        picks = input("Row numbers to compare, separated by commas: ")
        folders = [Path(table.loc[int(i), "path"]) for i in picks.split(",") if i.strip()]
    if not folders:
        raise RuntimeError("no recordings were chosen")
    _last_dir = root_dir
    return [load_recording(folder) for folder in folders]
